# ARION Pit Wall — driver app

Sends your Le Mans Ultimate telemetry live to the ARION pit wall, so coaches can follow your car.
Windows only. Needs Le Mans Ultimate 1.2 or newer.

## How to use it
1. In the ARION crew app: **Profile → Telemetry key → Make key → Copy.**
2. Download `ARION-Pit-Wall-vX.Y.Z.exe` from **Releases** and run it.
   Windows may warn that the app is from an unknown publisher: **More info → Run anyway.**
3. Paste the key and press **Save**. Done — from now on it just runs.

The window shows: connected to ARION (and your car), Le Mans Ultimate on track, how many coaches watch.
Closing the window keeps it running in the tray; to stop it: tray icon → **Quit**.
Tick **Start with Windows** if you want it ready every time.

**Save test report** writes a text file to your Desktop with measurements for the team (no personal data).

## Privacy
- The key stays on your PC, encrypted with your Windows account. The ARION database keeps only a scrambled
  (hashed) copy, so nobody can read your key there.
- The app sends telemetry only to coaches with pit wall access, directly from your PC to theirs (WebRTC).
- No SteamID, no IP address, no game files are stored by ARION. As with any direct connection, your PC and the
  coach's PC see each other's internet address while connected.

---

# ARION Pit Wall — εφαρμογή οδηγού

Στέλνει ζωντανά την τηλεμετρία σου από το Le Mans Ultimate στο pit wall της ARION, ώστε οι προπονητές να
βλέπουν το αυτοκίνητό σου. Μόνο για Windows, Le Mans Ultimate 1.2 ή νεότερο.

## Πώς τη χρησιμοποιείς
1. Στην εφαρμογή της ARION: **Προφίλ → Κλειδί τηλεμετρίας → Δημιουργία κλειδιού → Αντιγραφή.**
2. Κατέβασε το `ARION-Pit-Wall-vX.Y.Z.exe` από τα **Releases** και άνοιξέ το.
   Αν τα Windows προειδοποιήσουν για άγνωστο εκδότη: **Περισσότερες πληροφορίες → Εκτέλεση οπωσδήποτε.**
3. Επικόλλησε το κλειδί και πάτα **Αποθήκευση**. Τέλος — από εκεί και πέρα τρέχει μόνη της.

Το παράθυρο δείχνει: σύνδεση με την ARION (και το αυτοκίνητό σου), αν το Le Mans Ultimate είναι στην πίστα,
πόσοι προπονητές βλέπουν. Το κλείσιμο του παραθύρου την αφήνει να τρέχει στο tray· για διακοπή: εικονίδιο
tray → **Έξοδος**. Τσέκαρε **Εκκίνηση με τα Windows** αν τη θέλεις έτοιμη κάθε φορά.

Το κλειδί μένει στον υπολογιστή σου, κρυπτογραφημένο με τον λογαριασμό σου στα Windows· η βάση της ARION κρατά μόνο
μια ανακατεμένη (hash) μορφή του. Η τηλεμετρία πηγαίνει απευθείας από τον υπολογιστή σου στους προπονητές με
πρόσβαση στο pit wall (WebRTC)· όσο είναι συνδεδεμένοι, οι δύο υπολογιστές βλέπουν ο ένας τη διεύθυνση internet
του άλλου, όπως σε κάθε απευθείας σύνδεση. Η ARION δεν αποθηκεύει SteamID, διεύθυνση IP ή αρχεία του παιχνιδιού.

**Αποθήκευση αναφοράς δοκιμής**: γράφει ένα αρχείο κειμένου στην Επιφάνεια εργασίας με μετρήσεις για την
ομάδα (χωρίς προσωπικά στοιχεία).

---

For developers: `cargo test` (layout, protocol, report), `tests/e2e/run_e2e.py` (agent ⇄ mock database ⇄ six
headless Chrome coaches), `PROTOCOL.md` (what is sent), `THIRD-PARTY.md`.
