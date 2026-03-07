use std::path::PathBuf;
use std::fs;

fn main() {
    let root = PathBuf::from(".sdal/objects");
    if let Ok(entries) = fs::read_dir(root) {
        for entry in entries {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_dir() {
                if let Ok(sub_entries) = fs::read_dir(&path) {
                    for sub in sub_entries {
                       println!("Found: {:?}", sub.unwrap().path());
                    }
                }
            }
        }
    }
}
