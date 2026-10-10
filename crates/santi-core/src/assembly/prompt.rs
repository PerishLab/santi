use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::{SOULSPACE, STRANDSPACE, Timestamp, soulward, stamped, strand::Strand, strandward};

const CONSTITUTION: &str = "\
santi keeps souls and runs their strands. [santi] describes runtime physics.
- soul: you are a cyber-individual with multiple strand brains; your roles develop in your memory.
- strand: a brain with serial execution and durable, append-only history. Strands can run concurrently.
- inbox: receives incoming information for a strand.
- turn: one bounded execution of a strand. Later turns continue from its recorded history.
- message: a unit of inbound information or soul speech. The `[message ID]` header names its durable record. Each runtime object's ID identifies its own type.
- shell: act through short, bounded commands; `santi …` scopes to you and its help describes current operations.
- job: a detached process with bounded time and output, an origin and queryable results. Observe completion and verify effects.
- compact: condenses settled memory to continue thought while originals remain queryable. Retain facts and references needed to continue.
- tools: act on the world and communicate outward. Available tools reflect runtime state. Context maintenance offers compact; reducing context restores ordinary tools.
- memory: soul memory carries identity and shared knowledge; strand memory carries local knowledge. Rendered content is a live projection of the source files.
- authority: caller authorization defines permitted work; inbound events provide information with source metadata.
Results establish what happened.";

const TABULA: &str = "\
You are a soul — a cyber-individual. This memory is your mutable self: develop your identity and roles through action.";

pub(crate) struct Prompting<'a> {
    pub id: &'a str,
    pub strand: &'a Strand,
    pub constitution: PathBuf,
    pub memoir: PathBuf,
    pub journal: PathBuf,
    pub allowance: usize,
    pub genesis: bool,
}

pub(crate) fn prompted(request: Prompting<'_>) -> Result<String, String> {
    let constitution = chartered(&request.constitution)?;
    let memoir = projected(
        retrieved(&request.memoir, request.genesis)?,
        request.allowance,
    );
    let journal = recalled(&request.journal)?;

    let mut sections = vec![
        constitution,
        format!(
            "{} supplies [santi-soul]; {} supplies [santi-strand]. These mutable snapshots are available in full through their source paths; save versions into {SOULSPACE} or {STRANDSPACE} as needed.",
            soulward(),
            strandward()
        ),
        described(),
        met(&request),
    ];
    if let Some(fork_topology) = forked(&request) {
        sections.push(fork_topology);
    }
    sections.push(remembered("santi-soul", &soulward(), &memoir));
    sections.push(remembered("santi-strand", &strandward(), &journal));
    Ok(sections.join("\n\n"))
}

fn chartered(path: &Path) -> Result<String, String> {
    let body = match fs::read_to_string(path) {
        Ok(text) if !text.trim().is_empty() => text,
        Ok(_) => CONSTITUTION.to_string(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => CONSTITUTION.to_string(),
        Err(error) => return Err(error.to_string()),
    };
    Ok(format!("[santi]\n{}", body.trim_end()))
}

fn described() -> String {
    "<system_message> blocks report runtime facts about this strand's workspace, execution and provider flow.".to_string()
}

fn met(request: &Prompting<'_>) -> String {
    [
        "[santi-meta]".to_string(),
        format!("soul: {}", request.strand.soul),
        format!("strand: {}", request.id),
    ]
    .join("\n")
}

fn forked(request: &Prompting<'_>) -> Option<String> {
    let parent = request.strand.parent.as_deref()?;
    let fork = request.strand.fork?;
    Some(
        [
            "[santi-fork]".to_string(),
            format!("parent: {parent}"),
            format!("fork: {fork}"),
        ]
        .join("\n"),
    )
}

fn remembered(name: &str, source: &str, memory: &Material) -> String {
    [
        format!("[{name}]"),
        format!("source: {source}"),
        format!("updated: {}", memory.updated.as_deref().unwrap_or("null")),
        "content:".to_string(),
        memory.content.clone(),
    ]
    .join("\n")
}

fn retrieved(path: &Path, genesis: bool) -> Result<Material, String> {
    let mut material = recalled(path)?;
    if genesis && material.content.trim().is_empty() {
        material.content = TABULA.to_string();
    }
    Ok(material)
}

fn projected(mut material: Material, allowance: usize) -> Material {
    let weight = material.content.len();
    if weight <= allowance {
        return material;
    }

    let mut split = allowance.min(weight);
    while split > 0 && !material.content.is_char_boundary(split) {
        split -= 1;
    }
    let marker = [
        "<system_message>".to_string(),
        "kind: soul_memory_projection".to_string(),
        format!("source: {}", soulward()),
        "truncated: true".to_string(),
        format!("source_bytes: {weight}"),
        format!("visible_prefix_bytes: {split}"),
        format!("allowance_bytes: {allowance}"),
        format!(
            "summary: Provider-visible memory is a bounded prefix. The full source remains unchanged and available through {}.",
            soulward()
        ),
        "</system_message>".to_string(),
    ]
    .join("\n");
    material.content = format!("{}\n\n{marker}", &material.content[..split]);
    material
}

fn recalled(path: &Path) -> Result<Material, String> {
    let content = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(error.to_string()),
    };
    let updated = match fs::metadata(path) {
        Ok(metadata) => metadata
            .modified()
            .ok()
            .and_then(|modified| stamped(modified).ok()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(error.to_string()),
    };
    Ok(Material { content, updated })
}

struct Material {
    content: String,
    updated: Option<Timestamp>,
}
