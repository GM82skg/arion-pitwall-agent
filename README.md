# ARION Pit Wall — driver app

Sends your Le Mans Ultimate telemetry live to the ARION pit wall, so coaches can follow your car.
Windows only. Needs Le Mans Ultimate 1.2 or newer.

## How to use it
1. In the ARION crew app: **Profile → Telemetry key → Make key → Copy.**
2. Download `ARION-Pit-Wall-vX.Y.Z.exe` from **Releases** and run it.
   Windows may warn that the app is from an unknown publisher: **More info → Run anyway.**
3. Paste the key with **Paste** (or Ctrl+V and **Save**). Done — from now on it just runs.

**Streaming with OBS:** open **My dashboard**, click the chain icon at the top right of a panel, choose the background (0 % = see-through) and **Copy link**, then add it in OBS as a **Browser Source**. It shows that one panel.

**Updates:** the app checks for a newer version every 6 hours and shows **Update available**. **Update now** is only offered between sessions (Le Mans Ultimate closed, no coach watching): it downloads the new version, checks it against its SHA-256 and restarts. It never updates on its own.
To check right away: right-click the tray icon → **Check for updates**. The window opens and says whether you have the
latest version (or shows **Update available**).

### If Chrome or Windows warns
The app is new and not yet signed with a paid certificate, so Chrome and Windows don't know it yet and warn about
*every* program like that. The warning is about reputation, not about anything found in the file.
- **Chrome blocks the download:** open the downloads list (**Ctrl+J**) and choose to keep the file.
- **"Windows protected your PC":** **More info → Run anyway.**
- **To check the file yourself:** each release shows a SHA-256 code; paste it into virustotal.com → Search to see
  the results of about 70 antivirus scanners for exactly this file. The full source code is in this repository.
Download it only from this repository's **Releases** page.

The window shows: connected to ARION (and your car), Le Mans Ultimate on track, how many coaches watch.
Closing the window keeps it running in the tray. Left-click the tray icon to open the window again; right-click it for
**Show**, **Check for updates** and **Quit** (Quit stops sending).
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

**Για streaming με OBS:** άνοιξε το **My dashboard**, πάτα το εικονίδιο αλυσίδας πάνω δεξιά σε ένα πάνελ, διάλεξε το φόντο (0 % = διάφανο) και **Copy link**, και πρόσθεσέ το στο OBS ως **Browser Source**. Δείχνει μόνο εκείνο το πάνελ.

**Ενημερώσεις:** η εφαρμογή ελέγχει για νέα έκδοση κάθε 6 ώρες και δείχνει **Διαθέσιμη ενημέρωση**. Το **Ενημέρωση τώρα** εμφανίζεται μόνο ανάμεσα σε συνεδρίες (κλειστό Le Mans Ultimate, κανένας προπονητής): κατεβάζει τη νέα έκδοση, την ελέγχει με το SHA-256 της και επανεκκινεί. Δεν ενημερώνεται ποτέ μόνη της.
Για έλεγχο αμέσως: δεξί κλικ στο εικονίδιο του tray → **Έλεγχος για ενημερώσεις**. Ανοίγει το παράθυρο και λέει αν έχεις
την πιο πρόσφατη έκδοση (ή δείχνει **Διαθέσιμη ενημέρωση**).

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
πόσοι προπονητές βλέπουν. Το κλείσιμο του παραθύρου την αφήνει να τρέχει στο tray. Αριστερό κλικ στο εικονίδιο
του tray ξανανοίγει το παράθυρο· δεξί κλικ δίνει **Εμφάνιση**, **Έλεγχος για ενημερώσεις** και **Έξοδος** (η Έξοδος
σταματά την αποστολή). Τσέκαρε **Εκκίνηση με τα Windows** αν τη θέλεις έτοιμη κάθε φορά.

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

## For developers
- `cargo test`: LMU's memory layout checked field by field, the protocol, the fake source, penalties, updates, report.
- `tests/e2e/*.py`: the app ⇄ a stand-in database (`mock_db.py`) ⇄ headless Chrome coaches and the dashboard, plus
  `update_e2e.py` against a stand-in for GitHub, and layout audits (`overlap_audit.py`, `fit_audit.py`).
- Command line (tests and demos): `--key arion_tk_… --fake` streams a fake car (`--fake-speed 10`, `--fake-penalties`,
  `--fake-settings`); `--update` and `--check-updates` do what the window's «Update now» and the tray's «Check for
  updates» do.
- `PROTOCOL.md`: exactly what is sent (car frame with acceleration since v0.1.28; TC1/TC2/TC3 since v0.1.27).
  `THIRD-PARTY.md`: third-party notices. `assets/dashboard.html` is the pit wall dashboard built in for «My dashboard».
- Releases: push a tag `vX.Y.Z`; GitHub builds, tests and publishes the exe with its SHA-256.

## License
Copyright (C) 2026 George Makridis (GM82)

ARION Pit Wall is free software and licensed under the terms of the GNU General Public License as published by the
Free Software Foundation, either version 3 of the License, or (at your option) any later version. ARION Pit Wall is
distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY. See [LICENSE](./LICENSE) for more info.
Releases up to v0.1.28 were published under the MIT License, which still applies to those copies.

The ARION name, logo and wordmark (`assets/arion.ico`, `assets/icon64.rgba`, `assets/wordmark.rgba` and the wordmark in
`assets/dashboard.html`) are not covered by the GNU General Public License; they belong to the ARION team and may not
be used without its permission.

Licences and notices for third-party software are in [THIRD-PARTY.md](./THIRD-PARTY.md).

**Άδεια χρήσης:** Η ARION Pit Wall είναι ελεύθερο λογισμικό με άδεια GNU General Public License, έκδοση 3 ή (κατ'
επιλογή) νεότερη. Διανέμεται με την ελπίδα ότι θα είναι χρήσιμη, αλλά ΧΩΡΙΣ ΚΑΜΙΑ ΕΓΓΥΗΣΗ. Δες το [LICENSE](./LICENSE).
Το όνομα, το λογότυπο και το wordmark της ARION δεν καλύπτονται από την άδεια GPL· ανήκουν στην ομάδα ARION.
