use keel::{atom::string, resource};

#[resource]
pub(crate) struct Provider {
    #[field(string, unique)]
    name: string,
}

#[resource]
pub(crate) struct Family {
    #[field(string, unique)]
    name: string,
}

#[resource]
pub(crate) struct Scenario {
    #[field(string, unique)]
    name: string,
    #[field(string)]
    criterion: string,
}

#[resource]
pub(crate) struct Access {
    #[field(string, unique)]
    name: string,
    #[field(string)]
    protocol: string,
    #[field(string)]
    endpoint: string,
    #[field(string)]
    credential: string,
    #[relation(Provider, many2one)]
    provider: Provider,
}

#[resource]
pub(crate) struct Alias {
    #[field(string, unique)]
    name: string,
    #[relation(Access, many2one)]
    access: Access,
}

#[resource]
pub(crate) struct Offering {
    #[field(string, unique = access)]
    model: string,
    #[relation(Access, many2one)]
    access: Access,
    #[relation(Family, many2one)]
    family: Family,
}

#[resource]
pub(crate) struct Candidate {
    #[field(string)]
    evidence: string,
    #[relation(Scenario, many2one)]
    scenario: Scenario,
    #[relation(Offering, many2one)]
    offering: Offering,
}

#[resource]
pub(crate) struct Preference {
    #[relation(Candidate, many2one)]
    candidate: Candidate,
}
