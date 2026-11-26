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
    #[cascade(name = "encrypt_key")]
    pub secret: Option<String>,
    pub allow: Option<String>,
}
