use std::path::{Path, PathBuf};

pub(super) fn expand_home_template(template: &str, home_dir: &Path) -> PathBuf {
    let home_str = home_dir.display().to_string();
    PathBuf::from(template.replace("{home}", &home_str))
}
