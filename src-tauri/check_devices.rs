use evdev::Device;
use std::path::PathBuf;

fn main() {
    let dir = std::fs::read_dir("/dev/input").expect("read /dev/input");
    let mut paths: Vec<PathBuf> = dir
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with("event"))
                .unwrap_or(false)
        })
        .collect();
    paths.sort();
    
    for path in paths {
        match Device::open(&path) {
            Ok(dev) => {
                let name = dev.name().unwrap_or("(unknown)");
                println!("{} - OK: {}", path.display(), name);
            }
            Err(e) => {
                println!("{} - ERR: {}", path.display(), e);
            }
        }
    }
}
