//! The coaches connected to this driver: one WebRTC connection each, up to MAX_VIEWERS.
//! A coach (browser) makes the offer with two data channels:
//!   "fast" — unordered, no retransmits: 30 Hz telemetry; a late packet is dropped, never waited for
//!   "slow" — reliable: tyres, energy, laps, all cars, and the latency ping
//! This side answers once ICE gathering is complete (one answer message, no trickle), so the database
//! sees one message per coach. One coach failing or leaving never touches the others.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use bytes::BytesMut;
use serde_json::Value;
use webrtc::data_channel::{DataChannel, DataChannelEvent};
use webrtc::peer_connection::{
    PeerConnection, PeerConnectionBuilder, PeerConnectionEventHandler, RTCConfigurationBuilder, RTCIceGatheringState,
    RTCIceServer, RTCPeerConnectionState, RTCSessionDescription,
};
use webrtc::runtime::{channel, Sender};

use crate::config;

#[derive(Default)]
struct Viewer {
    pc: Option<Arc<dyn PeerConnection>>,
    fast: Option<Arc<dyn DataChannel>>,
    slow: Option<Arc<dyn DataChannel>>,
    fast_open: bool,
    slow_open: bool,
}

#[derive(Clone, Default)]
pub struct Hub {
    viewers: Arc<Mutex<HashMap<String, Viewer>>>,
}

struct Handler {
    hub: Hub,
    viewer: String,
    gathered: Sender<()>,
}

#[async_trait]
impl PeerConnectionEventHandler for Handler {
    async fn on_ice_gathering_state_change(&self, state: RTCIceGatheringState) {
        if state == RTCIceGatheringState::Complete {
            let _ = self.gathered.try_send(());
        }
    }
    async fn on_connection_state_change(&self, state: RTCPeerConnectionState) {
        if matches!(state, RTCPeerConnectionState::Failed | RTCPeerConnectionState::Closed) {
            self.hub.drop_viewer(&self.viewer);
        }
    }
    async fn on_data_channel(&self, dc: Arc<dyn DataChannel>) {
        let label = dc.label().await.unwrap_or_default();
        let fast = match label.as_str() {
            "fast" => true,
            "slow" => false,
            _ => return,
        };
        {
            let mut m = self.hub.viewers.lock().unwrap();
            let Some(v) = m.get_mut(&self.viewer) else { return };
            if fast { v.fast = Some(dc.clone()) } else { v.slow = Some(dc.clone()) }
        }
        let hub = self.hub.clone();
        let viewer = self.viewer.clone();
        tokio::spawn(async move {
            while let Some(ev) = dc.poll().await {
                match ev {
                    DataChannelEvent::OnOpen => hub.set_open(&viewer, fast, true),
                    DataChannelEvent::OnClose => {
                        hub.set_open(&viewer, fast, false);
                        break;
                    }
                    DataChannelEvent::OnMessage(msg) => {
                        // Latency ping from the coach: "p:<their clock>" → echo "q:<their clock>" at once.
                        if let Ok(t) = std::str::from_utf8(&msg.data) {
                            if let Some(rest) = t.strip_prefix("p:") {
                                let _ = dc.try_send_text(&format!("q:{rest}")).await;
                            }
                        }
                    }
                    _ => {}
                }
            }
        });
    }
}

impl Hub {
    pub fn new() -> Self {
        Self::default()
    }

    /// Coaches with an open fast channel.
    pub fn connected(&self) -> usize {
        self.viewers.lock().unwrap().values().filter(|v| v.fast_open).count()
    }
    /// Coaches connected or connecting.
    pub fn count(&self) -> usize {
        self.viewers.lock().unwrap().len()
    }
    pub fn has(&self, viewer: &str) -> bool {
        self.viewers.lock().unwrap().contains_key(viewer)
    }

    fn set_open(&self, viewer: &str, fast: bool, open: bool) {
        if let Some(v) = self.viewers.lock().unwrap().get_mut(viewer) {
            if fast { v.fast_open = open } else { v.slow_open = open }
        }
    }

    fn drop_viewer(&self, viewer: &str) -> Option<Arc<dyn PeerConnection>> {
        self.viewers.lock().unwrap().remove(viewer).and_then(|v| v.pc)
    }

    /// The coach left or sent "bye": close only their connection.
    pub async fn remove(&self, viewer: &str) {
        if let Some(pc) = self.drop_viewer(viewer) {
            let _ = pc.close().await;
        }
    }

    pub async fn close_all(&self) {
        let pcs: Vec<_> = self.viewers.lock().unwrap().drain().filter_map(|(_, v)| v.pc).collect();
        for pc in pcs {
            let _ = pc.close().await;
        }
    }

    /// Answer a coach's offer. Returns the answer as {"type":"answer","sdp":"…"} for the database.
    pub async fn answer(&self, viewer: &str, offer: &Value) -> Result<Value, String> {
        self.remove(viewer).await; // a coach reconnecting replaces their old connection
        if self.count() >= config::MAX_VIEWERS {
            return Err("full".into());
        }
        let offer: RTCSessionDescription = serde_json::from_value(offer.clone()).map_err(|e| format!("bad offer: {e}"))?;
        let (gathered_tx, mut gathered_rx) = channel::<()>(1);
        self.viewers.lock().unwrap().insert(viewer.to_string(), Viewer::default());
        let handler = Arc::new(Handler { hub: self.clone(), viewer: viewer.to_string(), gathered: gathered_tx });
        let cfg = RTCConfigurationBuilder::new()
            .with_ice_servers(vec![RTCIceServer { urls: vec![config::STUN_URL.to_string()], ..Default::default() }])
            .build();
        let built = PeerConnectionBuilder::new()
            .with_configuration(cfg)
            .with_handler(handler)
            .with_udp_addrs(vec!["0.0.0.0:0".to_string()])
            .build()
            .await;
        let pc: Arc<dyn PeerConnection> = match built {
            Ok(pc) => Arc::new(pc),
            Err(e) => {
                self.drop_viewer(viewer);
                return Err(format!("connection setup failed: {e}"));
            }
        };
        if let Some(v) = self.viewers.lock().unwrap().get_mut(viewer) {
            v.pc = Some(pc.clone());
        }
        let r: Result<Value, String> = async {
            pc.set_remote_description(offer).await.map_err(|e| e.to_string())?;
            let ans = pc.create_answer(None).await.map_err(|e| e.to_string())?;
            pc.set_local_description(ans).await.map_err(|e| e.to_string())?;
            // Wait for all network addresses (normally well under a second; STUN can take longer).
            let _ = tokio::time::timeout(Duration::from_secs(5), gathered_rx.recv()).await;
            let local = pc.local_description().await.ok_or("no local description")?;
            serde_json::to_value(&local).map_err(|e| e.to_string())
        }
        .await;
        if r.is_err() {
            self.remove(viewer).await;
        }
        r
    }

    fn channels(&self, fast: bool) -> Vec<Arc<dyn DataChannel>> {
        self.viewers
            .lock()
            .unwrap()
            .values()
            .filter(|v| if fast { v.fast_open } else { v.slow_open })
            .filter_map(|v| if fast { v.fast.clone() } else { v.slow.clone() })
            .collect()
    }

    /// Fast telemetry: never waits. A coach whose link is full simply misses this frame.
    pub async fn send_fast(&self, frame: &[u8]) {
        for dc in self.channels(true) {
            let _ = dc.try_send(BytesMut::from(frame)).await;
        }
    }
    /// Slow data: reliable and ordered.
    pub async fn send_slow(&self, text: &str) {
        for dc in self.channels(false) {
            let _ = dc.try_send_text(text).await;
        }
    }
}
