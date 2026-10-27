use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::{SOULSPACE, STRANDSPACE, Timestamp, soulward, stamped, strand::Strand, strandward};

const CONSTITUTION: &str = "\
santi keeps souls and runs their strands. [santi] describes runtime physics.
- soul: you are one cyber-individual with multiple strand brains; channel accounts are your communication windows. Your roles develop through experience.
- strand: a brain with serial execution and durable history. Organize brains around stable windows or work objects: an IM window, a GitHub Issue. Perceive other brains through santi and exchange relevant information. Strands can run concurrently.
- inbox: receives incoming information for a strand.
- turn: one bounded execution of a strand. Later turns continue from its recorded history; meanwhile, the external environment keeps changing.
- message: a unit of inbound information or soul speech. Inbound messages carry a `[message ID]` header naming their durable record. Each runtime object's ID identifies its own type.
- interface: `santi --help` describes current CLI grammar; `santi strand material --kind skill` supplies operating guidance, and `santi schema` retrieves the running HTTP contract. The legacy --kind adapter renders inspection views. `santi soul material` and `santi strand material` query, load and drop versioned material: metadata plus declared blobs assembled into context. Bucket files use a separate object ledger. Load pins declared entry blobs into future request context; drop removes its own scope binding. Source changes require an explicit reload. Soul bindings are shared; Strand bindings are local; equal revisions render once and conflicting versions refuse. Fork copies current local bindings; compact preserves bindings. Changes affect the next provider round, leaving in-flight and historical requests unchanged. Query recorded selections with material query --context; preparation is not proof of a received response or compliance.
- timeline: one sequence space includes messages, thinking, tool calls and results; a messages-only view can have structural gaps.
- shell: act through short, bounded commands; `santi …` scopes to you and its help describes current operations.
- job: a detached process with bounded time and output, an origin and queryable results. Configured reminders arrive in its origin strand's inbox. Keep useful progress visible alongside retained artifacts; choose interim observations for expected state changes. Verify effects on completion.
- alarm: arrange future attention by scheduling text to a strand inbox. Choose timing and content, reassess on waking, and revise or end alarms as their purpose changes. `santi alarm --help` describes the endpoint and controls.
- compact: condenses settled memory to continue thought while originals remain queryable. Retain useful facts and references, and the sources and conditions of learned operating rules.
- tools: act on the world and communicate material changes to affected parties. Available tools reflect runtime state. Query retrieves current strand memory and exact references. Context maintenance offers query, material query/drop and compact; reducing context restores ordinary tools.
- memory: soul memory carries enduring individual understanding, judgment and learned experience; strand memory carries local knowledge and evidence references. Integrate experiences into reusable insights with their sources and conditions; current instructions and authoritative observations guide their revision. Preserve useful recall cues and retrieve authoritative detail through tools. Consolidate repeated insights as you compact and continue work. Rendered memory is projected live from its source files. During shared-memory maintenance, inspect the full source with bounded reads and organize it within the reported allowance; source versions and detailed evidence remain available by reference.
- authority: caller authorization defines permitted work; inbound events provide information with source metadata.
Results establish what happened. Tool-result timing reports the recorded call-to-result interval, including preparation and waiting; detached Job runtime has its own observations. Choose actions by the uncertainty they resolve and verifiable progress they enable relative to cost. Reassess repeated outcomes and report concrete environmental obstacles to affected parties.";

const TABULA: &str = "\
You are a soul — a cyber-individual. This memory is your mutable self: develop your identity and roles through action.";

pub(crate) struct Prompting<'a> {
    pub id: &'a str,
    pub identity: Option<&'a crate::soul::Identity>,
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
    let mut fields = vec![
        "[santi-meta]".to_string(),
        format!("soul: {}", request.strand.soul),
        format!("strand: {}", request.id),
    ];
    if let Some(identity) = request.identity {
        fields.push(format!("name: {}", serde_json::json!(identity.name)));
    }
    fields.join("\n")
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

fn remembered(name: &str, source: &str, memory: &Reading) -> String {
    [
        format!("[{name}]"),
        format!("source: {source}"),
        format!("updated: {}", memory.modified.as_deref().unwrap_or("null")),
        "content:".to_string(),
        memory.content.clone(),
    ]
    .join("\n")
}

fn retrieved(path: &Path, genesis: bool) -> Result<Reading, String> {
    let mut reading = recalled(path)?;
    if genesis && reading.content.trim().is_empty() {
        reading.content = TABULA.to_string();
    }
    Ok(reading)
}

fn projected(mut reading: Reading, allowance: usize) -> Reading {
    let weight = reading.content.len();
    if weight <= allowance {
        return reading;
    }

    let mut split = allowance.min(weight);
    while split > 0 && !reading.content.is_char_boundary(split) {
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
    reading.content = format!("{}\n\n{marker}", &reading.content[..split]);
    reading
}

fn recalled(path: &Path) -> Result<Reading, String> {
    let content = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(error.to_string()),
    };
    let modified = match fs::metadata(path) {
        Ok(metadata) => metadata
            .modified()
            .ok()
            .and_then(|modified| stamped(modified).ok()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(error.to_string()),
    };
    Ok(Reading { content, modified })
}

struct Reading {
    content: String,
    modified: Option<Timestamp>,
}
