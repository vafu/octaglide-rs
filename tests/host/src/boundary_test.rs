use std::{fs, path::Path};

const FORBIDDEN: [&str; 6] = [
    "teensy4_bsp",
    "teensy_usbhost",
    "cortex_m",
    "rtic::app",
    "rtic_monotonics",
    "tokio::",
];

fn scan(dir: &str) -> Vec<String> {
    let mut violations = Vec::new();
    let mut stack = vec![Path::new(dir).to_path_buf()];
    while let Some(path) = stack.pop() {
        let Ok(entries) = fs::read_dir(&path) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                let Ok(source) = fs::read_to_string(&path) else {
                    continue;
                };
                for needle in FORBIDDEN {
                    if source.contains(needle) {
                        violations.push(format!("{} contains {needle}", path.display()));
                    }
                }
            }
        }
    }
    violations
}

#[test]
fn portable_modules_have_no_platform_imports() {
    let violations: Vec<String> = scan("../../src/core")
        .into_iter()
        .chain(scan("../../src/runtime"))
        .collect();
    assert!(violations.is_empty(), "{violations:#?}");
}
