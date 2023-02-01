pub enum SubscribeTopic {
    StateVector(String),
    UpdateWithSV(String),
    OnUpdate(String),
    OnAwarenessUpdate(String),
}

impl ToString for SubscribeTopic {
    fn to_string(&self) -> String {
        match self {
            Self::StateVector(workspace) => format!("keck/{workspace:}/StateVector"),
            Self::UpdateWithSV(workspace) => format!("keck/{workspace:}/UpdateWithSV"),
            Self::OnUpdate(workspace) => format!("keck/{workspace:}/OnUpdate"),
            Self::OnAwarenessUpdate(workspace) => format!("keck/{workspace:}/OnAwarenessUpdate"),
        }
    }
}

impl TryFrom<&str> for SubscribeTopic {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let (workspace, topic) = value
            .strip_prefix("keck/")
            .ok_or(format!("unknown topic namespace: {}", value))?
            .split_once('/')
            .ok_or(format!("unknown topic: {}", value))?;

        match topic {
            "StateVector" => Ok(Self::StateVector(workspace.to_string())),
            "UpdateWithSV" => Ok(Self::UpdateWithSV(workspace.to_string())),
            "OnUpdate" => Ok(Self::OnUpdate(workspace.to_string())),
            "OnAwarenessUpdate" => Ok(Self::OnAwarenessUpdate(workspace.to_string())),
            _ => Err(format!("Unknown topic: {}", value)),
        }
    }
}
