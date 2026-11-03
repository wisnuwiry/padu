use serde::Deserialize;
use std::{collections::BTreeMap, fs, path::Path};

#[derive(Debug, Deserialize)]
struct Frontmatter {
    name: String,
    description: String,
}

pub fn generate(root: &Path) -> Result<String, String> {
    let mut resources = BTreeMap::new();
    if !root.exists() {
        return Ok("static BUILTINS: &[Resource] = &[];".to_string());
    }
    for entry in fs::read_dir(root).map_err(|e| e.to_string())? {
        let path = entry.map_err(|e| e.to_string())?.path();
        if !path.is_file() || path.extension().and_then(|ext| ext.to_str()) != Some("md") {
            continue;
        }
        let file = path;
        let source = fs::read_to_string(&file).map_err(|e| format!("{}: {e}", file.display()))?;
        let source = source.replace("\r\n", "\n");
        let (meta, _body) = parse_frontmatter(&source)
            .ok_or_else(|| format!("{}: missing or invalid YAML frontmatter", file.display()))?;
        let name = meta.name.trim().to_string();
        if name.is_empty() || file.file_stem().and_then(|stem| stem.to_str()) != Some(name.as_str())
        {
            return Err(format!(
                "{}: invalid name or filename mismatch with {name}",
                file.display()
            ));
        }
        let description = meta.description.trim().to_string();
        if description.is_empty() {
            return Err(format!("{}: description must not be empty", file.display()));
        }
        let entry = format!(
            "Resource {{ name: {:?}, description: {:?}, content: {:?} }}",
            name, description, source,
        );
        if resources.insert(name, entry).is_some() {
            return Err("duplicate resource name".to_owned());
        }
    }
    Ok(format!(
        "static BUILTINS: &[Resource] = &[{}];",
        resources.into_values().collect::<Vec<_>>().join(",\n")
    ))
}

fn parse_frontmatter(source: &str) -> Option<(Frontmatter, &str)> {
    let rest = source.strip_prefix("---\n")?;
    let end_idx = rest.find("\n---\n")?;
    let yaml = &rest[..end_idx];
    let body = &rest[end_idx + 5..];
    let meta: Frontmatter = serde_saphyr::from_str(yaml).ok()?;
    Some((meta, body))
}
