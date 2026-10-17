use simple_ip_core::{ChangePlan, EditorDraft, SampleAdapter, ValidationIssue, sample_adapters};

pub struct Session {
    pub draft: EditorDraft,
    pub adapters: Vec<SampleAdapter>,
    pub selected_adapter: usize,
    pub preview: Option<ChangePlan>,
    pub issues: Vec<ValidationIssue>,
    /// False while showing built-in sample adapters instead of this machine's.
    pub live: bool,
}

impl Default for Session {
    fn default() -> Self {
        let adapters = sample_adapters();
        Self {
            draft: EditorDraft::from_config(&adapters[0].config),
            adapters,
            selected_adapter: 0,
            preview: None,
            issues: vec![],
            live: false,
        }
    }
}

impl Session {
    /// Replaces the adapter snapshot, keeping the selected adapter and the user's edits.
    pub fn load_adapters(&mut self, adapters: Vec<SampleAdapter>) {
        if adapters.is_empty() {
            return;
        }
        let dirty = self.is_dirty();
        let selected = self.adapters[self.selected_adapter].id;
        let first_load = !self.live;
        self.adapters = adapters;
        self.live = true;
        self.selected_adapter = self
            .adapters
            .iter()
            .position(|a| a.id == selected && !first_load)
            .unwrap_or(0);
        if first_load || !dirty {
            self.reset_draft();
        } else {
            self.invalidate_preview();
        }
    }

    /// Starts the form over from the selected adapter's current configuration.
    pub fn reset_draft(&mut self) {
        self.draft = EditorDraft::from_config(&self.adapters[self.selected_adapter].config);
        self.invalidate_preview();
    }

    pub fn is_dirty(&self) -> bool {
        self.draft != EditorDraft::from_config(&self.adapters[self.selected_adapter].config)
    }

    pub fn draft_changed(&mut self) {
        self.preview = None;
        if !self.issues.is_empty() {
            self.issues = self.draft.validate().err().unwrap_or_default();
        }
    }

    pub fn draft(&self) -> &EditorDraft {
        &self.draft
    }

    pub fn draft_mut(&mut self) -> &mut EditorDraft {
        &mut self.draft
    }

    /// Fixed manual values used by tests and capture scenarios.
    #[allow(dead_code)]
    pub fn load_lab_sample(&mut self) {
        self.copy_current();
        self.draft.address = "198.51.100.10".into();
        self.draft.gateway.clear();
        self.draft.preferred_dns = "198.51.100.53".into();
    }

    pub fn invalidate_preview(&mut self) {
        self.preview = None;
        self.issues.clear();
    }

    pub fn make_preview(&mut self) -> bool {
        self.invalidate_preview();
        match self.draft.validate() {
            Ok(config) => {
                self.preview = Some(ChangePlan::preview(
                    &self.adapters[self.selected_adapter],
                    &config,
                ));
                true
            }
            Err(issues) => {
                self.issues = issues;
                false
            }
        }
    }

    pub fn copy_current(&mut self) -> bool {
        let adapter = &self.adapters[self.selected_adapter];
        let Some(address) = adapter.address else {
            return false;
        };
        let draft = self.draft_mut();
        // Copy observed addresses as explicit static values.
        draft.automatic_ip = false;
        draft.address = address.to_string();
        let adapter = &self.adapters[self.selected_adapter];
        let subnet = simple_ip_core::prefix_mask(adapter.prefix).to_string();
        let gateway = adapter.gateway.map(|v| v.to_string()).unwrap_or_default();
        let preferred = adapter
            .dns
            .first()
            .map(ToString::to_string)
            .unwrap_or_default();
        let alternate = adapter
            .dns
            .get(1)
            .map(ToString::to_string)
            .unwrap_or_default();
        let draft = self.draft_mut();
        draft.subnet = subnet;
        draft.gateway = gateway;
        draft.automatic_dns = false;
        draft.preferred_dns = preferred;
        draft.alternate_dns = alternate;
        self.invalidate_preview();
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validation_follows_edits_and_reset_matches_adapter() {
        let mut session = Session::default();
        session.draft.automatic_ip = false;
        session.draft.subnet = "bad".into();
        assert!(!session.make_preview());
        session.draft.address = "192.0.2.10".into();
        session.draft_changed();
        assert!(session.issues.iter().any(|i| i.field == "Subnet mask"));
        assert!(session.is_dirty());
        session.reset_draft();
        assert!(!session.is_dirty());
        assert!(session.preview.is_none() && session.issues.is_empty());
    }

    #[test]
    fn live_snapshot_replaces_samples_and_keeps_later_edits() {
        let mut session = Session::default();
        let mut live = sample_adapters();
        live.swap(0, 2);
        session.load_adapters(live.clone());
        assert!(session.live && !session.is_dirty());
        assert_eq!(session.adapters[0].name, "USB Ethernet");
        session.selected_adapter = 1;
        session.draft.address = "198.51.100.77".into();
        session.draft.automatic_ip = false;
        live.swap(0, 1);
        session.load_adapters(live);
        assert_eq!(session.adapters[session.selected_adapter].name, "Wi-Fi");
        assert_eq!(session.draft.address, "198.51.100.77");
    }

    #[test]
    fn preview_never_changes_adapter() {
        let mut session = Session::default();
        session.copy_current();
        let config = session.adapters[0].config.clone();
        assert!(session.make_preview());
        assert_eq!(session.adapters[0].config, config);
    }

    #[test]
    fn copy_current_is_manual_and_skips_unaddressed_adapter() {
        let mut session = Session::default();
        assert!(session.copy_current());
        assert!(!session.draft.automatic_ip);
        assert!(!session.draft.automatic_dns);
        session.selected_adapter = 1;
        let before = session.draft.clone();
        assert!(!session.copy_current());
        assert_eq!(session.draft, before);
    }
}
