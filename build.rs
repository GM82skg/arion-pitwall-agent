// Windows only: puts the icon and file details (name, company, version, description) into the .exe,
// so Windows shows "ARION Pit Wall" in Properties > Details, Task Manager and the taskbar.
fn main() {
    println!("cargo:rerun-if-changed=assets/arion.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut r = winresource::WindowsResource::new();
        r.set_icon("assets/arion.ico")
            .set("ProductName", "ARION Pit Wall")
            .set("FileDescription", "ARION Pit Wall - driver telemetry app")
            .set("CompanyName", "ARION")
            .set("LegalCopyright", "ARION")
            .set("OriginalFilename", "ARION-Pit-Wall.exe")
            .set("InternalName", "arion-pitwall-agent");
        r.compile().expect("could not add the icon and file details");
    }
}
