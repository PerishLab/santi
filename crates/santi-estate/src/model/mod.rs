use keel::Graph;

mod alarm;
use ops::catalog;
mod edge;
mod history;
mod inbox;
mod job;
mod ledger;
mod material;
mod ops;
mod wake;

pub(super) fn graph() -> Graph {
    let mut graph = Graph::new();
    ledger::plug(&mut graph);
    job::plug(&mut graph);
    inbox::plug(&mut graph);
    history::plug(&mut graph);
    edge::plug(&mut graph);
    ops::plug(&mut graph);
    graph.add(wake::lease());
    alarm::plug(&mut graph);
    graph
        .plug::<material::Revision>()
        .plug::<material::Blob>()
        .plug::<material::Declaration>()
        .plug::<material::Change>()
        .plug::<material::Shared>()
        .plug::<material::Local>()
        .plug::<material::Context>()
        .plug::<material::Usage>();
    graph
        .plug::<catalog::Provider>()
        .plug::<catalog::Family>()
        .plug::<catalog::Scenario>()
        .plug::<catalog::Access>()
        .plug::<catalog::Alias>()
        .plug::<catalog::Offering>()
        .plug::<catalog::Candidate>()
        .plug::<catalog::Preference>();
    graph
}
