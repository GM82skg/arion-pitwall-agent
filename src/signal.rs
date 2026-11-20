//! The only door to the database: rpc/pitwall_agent(key, action, data) (migration 58).
//! hello → session and car · poll → coach messages · send → answer/bye to one coach · bye → leave.

use serde_json::{json, Value};

pub trait Door: Send + Sync {
    fn call(&self, action: &str, data: Value) -> Result<Value, String>;
}

pub struct Supabase {
    url: String,
    apikey: String,
    key: String,
    agent: ureq::Agent,
}

impl Supabase {
    pub fn new(url: &str, apikey: &str, telemetry_key: &str) -> Self {
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_global(Some(std::time::Duration::from_secs(10)))
            .http_status_as_error(false)
            .build()
            .into();
        Supabase { url: url.trim_end_matches('/').to_string(), apikey: apikey.into(), key: telemetry_key.trim().into(), agent }
    }
}

impl Door for Supabase {
    fn call(&self, action: &str, data: Value) -> Result<Value, String> {
        if std::env::var("ARION_DEBUG").is_ok() { eprintln!("call {action} {:?}", std::time::SystemTime::now()); }
        let body = json!({ "p_key": self.key, "p_action": action, "p_data": data });
        let mut res = self
            .agent
            .post(&format!("{}/rest/v1/rpc/pitwall_agent", self.url))
            .header("apikey", &self.apikey)
            .header("Content-Type", "application/json")
            .send_json(&body)
            .map_err(|_| "No connection to the ARION server.".to_string())?;
        let status = res.status().as_u16();
        let v: Value = res.body_mut().read_json().unwrap_or(Value::Null);
        if (200..300).contains(&status) {
            Ok(v)
        } else {
            // PostgREST error: {"message": "..."}; the database messages are written for people.
            Err(v.get("message").and_then(|m| m.as_str()).unwrap_or("The server refused the request.").to_string())
        }
    }
}

/// What hello returns.
#[derive(Debug, Clone, Default)]
pub struct Hello {
    pub session: String,
    pub name: String,
    pub team: Option<Team>,
}
#[derive(Debug, Clone, PartialEq)]
pub struct Team {
    pub name: String,
    pub class: String,
    pub race: String,
}

pub fn parse_team(v: &Value) -> Option<Team> {
    let t = v.as_object()?;
    Some(Team {
        name: t.get("name")?.as_str()?.to_string(),
        class: t.get("class").and_then(|x| x.as_str()).unwrap_or("").to_string(),
        race: t.get("race").and_then(|x| x.as_str()).unwrap_or("").to_string(),
    })
}

pub fn parse_hello(v: &Value) -> Result<Hello, String> {
    Ok(Hello {
        session: v.get("session").and_then(|s| s.as_str()).ok_or("Unexpected reply from the server.")?.to_string(),
        name: v.get("name").and_then(|s| s.as_str()).unwrap_or("Driver").to_string(),
        team: v.get("team").and_then(parse_team),
    })
}

/// One coach message from poll.
#[derive(Debug, Clone)]
pub struct Signal {
    pub id: i64,
    pub viewer: String,
    pub kind: String,
    pub body: Value,
}

pub fn parse_poll(v: &Value) -> (Option<Team>, Vec<Signal>) {
    let team = v.get("team").and_then(parse_team);
    let sigs = v
        .get("signals")
        .and_then(|s| s.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|s| {
                    Some(Signal {
                        id: s.get("id")?.as_i64()?,
                        viewer: s.get("viewer")?.as_str()?.to_string(),
                        kind: s.get("kind")?.as_str()?.to_string(),
                        body: s.get("body").cloned().unwrap_or(Value::Null),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    (team, sigs)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_replies() {
        let h = parse_hello(&json!({"session":"s1","name":"DeeDee","team":{"id":1,"name":"Arion #82","class":"GT3","car":null,"race":"Monza"}})).unwrap();
        assert_eq!(h.session, "s1");
        assert_eq!(h.team, Some(Team { name: "Arion #82".into(), class: "GT3".into(), race: "Monza".into() }));
        let h = parse_hello(&json!({"session":"s2","name":"X","team":null})).unwrap();
        assert!(h.team.is_none());
        assert!(parse_hello(&json!({})).is_err());
        let (t, s) = parse_poll(&json!({"team":null,"signals":[{"id":5,"viewer":"v","kind":"offer","body":{"type":"offer","sdp":"x"}},{"bad":1}]}));
        assert!(t.is_none());
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].id, 5);
    }
}
