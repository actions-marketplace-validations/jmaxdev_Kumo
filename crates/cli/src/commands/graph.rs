use anyhow::Result;
use resolver::Lockfile;

pub async fn execute() -> Result<()> {
    let lock_path = std::env::current_dir()?.join("kumo.lock");
    if !lock_path.exists() {
        anyhow::bail!("kumo.lock not found.");
    }
    let lockfile: Lockfile = serde_yml::from_str(&std::fs::read_to_string(lock_path)?)?;

    let mut dot = String::from("digraph G {\n");
    dot.push_str("  node [shape=box, fontname=\"Arial\"];\n");

    for (name, version) in &lockfile.dependencies {
        dot.push_str(&format!("  \"Project\" -> \"{}@{}\";\n", name, version));
    }

    for (key, pkg) in &lockfile.packages {
        if let Some(deps) = &pkg.dependencies {
            for (d_name, d_range) in deps {
                let mut d_key = format!("{}@{}", d_name, d_range);
                for k in lockfile.packages.keys() {
                    if k.starts_with(d_name) {
                        d_key = k.clone();
                        break;
                    }
                }
                dot.push_str(&format!("  \"{}\" -> \"{}\";\n", key, d_key));
            }
        }
    }

    dot.push_str("}\n");
    std::fs::write("dependency-graph.dot", dot)?;
    println!("Graph saved to dependency-graph.dot. Use 'dot -Tsvg dependency-graph.dot -o graph.svg' to visualize.");
    Ok(())
}
