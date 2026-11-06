use radicula_config::Cascade;

#[derive(Debug, Default, Cascade)]
#[cascade(section)]
pub struct Webhooks {
    #[cascade(section)]
    pub github: Github,
    #[cascade(section)]
    pub feishu: Feishu,
}

#[derive(Debug, Default, Cascade)]
#[cascade(section)]
pub struct Github {
    pub login: Option<String>,
    pub allow: Option<String>,
}

#[derive(Debug, Default, Cascade)]
#[cascade(section)]
pub struct Feishu {
    pub acknowledge: Option<String>,
    pub feedback: Option<String>,
    pub application: Option<String>,
    pub encrypt_key: Option<String>,
    pub allow: Option<String>,
}
