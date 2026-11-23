# ARION Pit Wall — driver app

Sends your Le Mans Ultimate telemetry live to the ARION pit wall, so coaches can follow your car.
Windows only. Needs Le Mans Ultimate 1.2 or newer.

## How to use it
1. In the ARION crew app: **Profile → Telemetry key → Make key → Copy.**
2. Download `ARION-Pit-Wall-vX.Y.Z.exe` from **Releases** and run it.
   Windows may warn that the app is from an unknown publisher: **More info → Run anyway.**
3. Paste the key with **Paste** (or Ctrl+V and **Save**). Done — from now on it just runs.

**Streaming with OBS:** open **My dashboard**, click the chain icon at the top right of a panel, and add the copied link in OBS as a **Browser Source**. It shows that one panel on a transparent background.

### If Chrome or Windows warns
The app is new and not yet signed with a paid certificate, so Chrome and Windows don't know it yet and warn about
*every* program like that. The warning is about reputation, not about anything found in the file.
- **Chrome blocks the download:** open the downloads list (**Ctrl+J**) and choose to keep the file.
- **"Windows protected your PC":** **More info → Run anyway.**
- **To check the file yourself:** each release shows a SHA-256 code; paste it into virustotal.com → Search to see
  the results of about 70 antivirus scanners for exactly this file. The full source code is in this repository.
Download it only from this repository's **Releases** page.

The window shows: connected to ARION (and your car), Le Mans Ultimate on track, how many coaches watch.
Closing the window keeps it running in the tray; to stop it: tray icon → **Quit**.
Tick **Start with Windows** if you want it ready every time.

**Open my dashboard** shows your own car on this PC (map, inputs, tyres, traffic) in your browser — for example on a
second monitor. It works only on this PC, needs no login, and doesn't use one of the coaches' places.

**Save test report** writes a text file to your Desktop with measurements for the team (no personal data).

## Privacy
- The key stays on your PC, encrypted with your Windows account. The ARION database keeps only a scrambled
  (hashed) copy, so nobody can read your key there.
- The app sends telemetry only to coaches with pit wall access, directly from your PC to theirs (WebRTC).
- Your laps, and the best lap of each other car in your class, are saved on your own PC for lap comparisons
  (folder `ARION Pit Wall\laps` in your AppData). They are not uploaded by this version.
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
3. Πάτα **Επικόλληση** (ή Ctrl+V και **Αποθήκευση**). Τέλος — από εκεί και πέρα τρέχει μόνη της.

**Για streaming με OBS:** άνοιξε το **My dashboard**, πάτα το εικονίδιο αλυσίδας πάνω δεξιά σε ένα πάνελ και πρόσθεσε τον σύνδεσμο που αντιγράφηκε στο OBS ως **Browser Source**. Δείχνει μόνο εκείνο το πάνελ, με διάφανο φόντο.

### Αν το Chrome ή τα Windows δείξουν προειδοποίηση
Η εφαρμογή είναι καινούργια και δεν έχει ακόμα πληρωμένη ψηφιακή υπογραφή, οπότε το Chrome και τα Windows δεν την
«γνωρίζουν» και προειδοποιούν για *κάθε* τέτοιο πρόγραμμα. Η προειδοποίηση αφορά τη φήμη του αρχείου, όχι κάτι που
βρέθηκε μέσα του.
- **Το Chrome μπλοκάρει τη λήψη:** άνοιξε τις λήψεις (**Ctrl+J**) και επίλεξε να κρατήσεις το αρχείο.
- **«Τα Windows προστάτευσαν τον υπολογιστή σας»:** **Περισσότερες πληροφορίες → Εκτέλεση οπωσδήποτε.**
- **Για να το ελέγξεις μόνος σου:** κάθε έκδοση δείχνει έναν κωδικό SHA-256· επικόλλησέ τον στο virustotal.com →
  Search για να δεις τα αποτελέσματα ~70 antivirus για ακριβώς αυτό το αρχείο. Ο πλήρης κώδικας είναι σε αυτό το repository.
Κατέβαζέ την μόνο από τη σελίδα **Releases** αυτού του repository.

Το παράθυρο δείχνει: σύνδεση με την ARION (και το αυτοκίνητό σου), αν το Le Mans Ultimate είναι στην πίστα,
πόσοι προπονητές βλέπουν. Το κλείσιμο του παραθύρου την αφήνει να τρέχει στο tray· για διακοπή: εικονίδιο
tray → **Έξοδος**. Τσέκαρε **Εκκίνηση με τα Windows** αν τη θέλεις έτοιμη κάθε φορά.

Το κλειδί μένει στον υπολογιστή σου, κρυπτογραφημένο με τον λογαριασμό σου στα Windows· η βάση της ARION κρατά μόνο
μια ανακατεμένη (hash) μορφή του. Η τηλεμετρία πηγαίνει απευθείας από τον υπολογιστή σου στους προπονητές με
πρόσβαση στο pit wall (WebRTC)· όσο είναι συνδεδεμένοι, οι δύο υπολογιστές βλέπουν ο ένας τη διεύθυνση internet
του άλλου, όπως σε κάθε απευθείας σύνδεση. Οι γύροι σου, και ο καλύτερος γύρος κάθε άλλου αυτοκινήτου της κατηγορίας σου, αποθηκεύονται στον υπολογιστή σου για
συγκρίσεις (φάκελος `ARION Pit Wall\laps` στο AppData)· αυτή η έκδοση δεν τους ανεβάζει πουθενά. Η ARION δεν αποθηκεύει SteamID, διεύθυνση IP ή αρχεία του παιχνιδιού.

**Άνοιγμα του dashboard μου**: δείχνει το δικό σου αυτοκίνητο σε αυτόν τον υπολογιστή (χάρτης, πεντάλ, ελαστικά,
κίνηση) στον browser — π.χ. σε δεύτερη οθόνη. Λειτουργεί μόνο σε αυτόν τον υπολογιστή, χωρίς σύνδεση λογαριασμού, και
δεν πιάνει θέση προπονητή.

**Αποθήκευση αναφοράς δοκιμής**: γράφει ένα αρχείο κειμένου στην Επιφάνεια εργασίας με μετρήσεις για την
ομάδα (χωρίς προσωπικά στοιχεία).

---

For developers: `cargo test` (layout, protocol, report), `tests/e2e/run_e2e.py` (agent ⇄ mock database ⇄ six
headless Chrome coaches), `PROTOCOL.md` (what is sent), `THIRD-PARTY.md`.
