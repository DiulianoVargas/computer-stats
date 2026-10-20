//! The Processes View's UI-thread state: the row model (updated in place), icons, order, and the
//! End App flow (003 FR-008–FR-017; research R8–R10).

use std::collections::HashMap;
use std::rc::Rc;
use std::time::{Duration, Instant};

use slint::{Image, Model, ModelRc, Rgba8Pixel, SharedPixelBuffer, VecModel};

use crate::domain::app_order::{self, Sort};
use crate::domain::apps::{AppKey, AppUsage};
use crate::domain::end_confirm::{ARM_TIME, EndConfirm};
use crate::domain::format;
use crate::generated::AppRowData;
use crate::platform::processes;
use crate::process_sampler::ProcessesSample;

/// How long a row shows a message such as "access denied", or stays disabled after End.
const ROW_FEEDBACK: Duration = Duration::from_secs(3);

/// What an End click led to.
pub enum EndResult {
    /// Nothing ended (armed, re-armed, or the App is gone).
    None,
    /// The armed App is Computer Stats itself: the caller quits (FR-017).
    Quit,
}

pub struct ProcessesUi {
    model: Rc<VecModel<AppRowData>>,
    sort: Sort,
    /// Display order of the last refresh, for the hover freeze (FR-010).
    order: Vec<AppKey>,
    icons: HashMap<AppKey, Image>,
    latest: Option<ProcessesSample>,
    confirm: EndConfirm,
    /// Rows recently Ended (disabled) or showing a message, until the given time.
    ending: HashMap<AppKey, Instant>,
    messages: HashMap<AppKey, (String, Instant)>,
}

impl ProcessesUi {
    pub fn new() -> ProcessesUi {
        ProcessesUi {
            model: Rc::new(VecModel::default()),
            sort: Sort::default(),
            order: Vec::new(),
            icons: HashMap::new(),
            latest: None,
            confirm: EndConfirm::default(),
            ending: HashMap::new(),
            messages: HashMap::new(),
        }
    }

    pub fn model(&self) -> ModelRc<AppRowData> {
        ModelRc::from(self.model.clone())
    }

    pub fn sort(&self) -> Sort {
        self.sort
    }

    /// A new sample: cache its icons and refresh the rows (order frozen while `hovered`).
    pub fn apply(&mut self, sample: ProcessesSample, hovered: bool) {
        for (key, icon) in &sample.icons {
            let buffer =
                SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(&icon.rgba, icon.size, icon.size);
            self.icons.insert(key.clone(), Image::from_rgba8(buffer));
        }
        let running: Vec<&AppKey> = sample.apps.iter().map(|a| &a.key).collect();
        self.icons.retain(|k, _| running.contains(&k));
        self.latest = Some(sample);
        self.refresh(hovered);
    }

    pub fn set_sort(&mut self, sort: Sort) {
        self.sort = sort;
        self.refresh(false);
    }

    /// Everything is dropped when the View is hidden (FR-018); the sort is kept (FR-002).
    pub fn clear(&mut self) {
        self.confirm.cancel();
        self.latest = None;
        self.order.clear();
        self.icons.clear();
        self.ending.clear();
        self.messages.clear();
        self.model.set_vec(Vec::new());
    }

    pub fn cancel_arming(&mut self) {
        if self.confirm.armed().is_some() {
            self.confirm.cancel();
            self.refresh(true);
        }
    }

    /// Expires arming, row messages and Ending marks; called by a timer after `ARM_TIME`.
    pub fn expire(&mut self) {
        let now = Instant::now();
        self.confirm.expire(now);
        self.ending.retain(|_, until| *until > now);
        self.messages.retain(|_, (_, until)| *until > now);
        self.refresh(true);
    }

    /// An End button click on `key` (FR-012–FR-016). The caller schedules `expire` after
    /// `ARM_TIME` and handles `EndResult::Quit`.
    pub fn end_clicked(&mut self, key: &str) -> EndResult {
        let now = Instant::now();
        let Some(armed) = self.confirm.click(key, now) else {
            self.refresh(true);
            return EndResult::None;
        };
        let Some(sample) = &self.latest else {
            return EndResult::None;
        };
        let Some(app) = sample.apps.iter().find(|a| a.key == armed) else {
            self.refresh(true);
            return EndResult::None;
        };
        if app.processes.iter().any(|p| p.pid == sample.own_pid) {
            return EndResult::Quit;
        }
        let outcome = processes::end(&app.processes);
        if outcome.denied > 0 {
            self.messages
                .insert(armed.clone(), ("access denied".into(), now + ROW_FEEDBACK));
        }
        if outcome.ended > 0 {
            self.ending.insert(armed, now + ROW_FEEDBACK);
        }
        self.refresh(true);
        EndResult::None
    }

    /// Rebuilds the rows from the latest sample and updates the model in place, so the list
    /// keeps its scroll position (FR-011).
    fn refresh(&mut self, frozen: bool) {
        let Some(sample) = &self.latest else {
            return;
        };
        let indices = app_order::order(
            &sample.apps,
            self.sort,
            frozen.then_some(self.order.as_slice()),
        );
        self.order = indices
            .iter()
            .map(|&i| sample.apps[i].key.clone())
            .collect();
        let rows: Vec<AppRowData> = indices.iter().map(|&i| self.row(&sample.apps[i])).collect();

        for (i, row) in rows.iter().enumerate() {
            if i < self.model.row_count() {
                if self.model.row_data(i).as_ref() != Some(row) {
                    self.model.set_row_data(i, row.clone());
                }
            } else {
                self.model.push(row.clone());
            }
        }
        while self.model.row_count() > rows.len() {
            self.model.remove(self.model.row_count() - 1);
        }
    }

    fn row(&self, app: &AppUsage) -> AppRowData {
        let icon = self.icons.get(&app.key);
        AppRowData {
            key: app.key.as_str().into(),
            name: format::app_name_with_count(&app.name, app.processes.len()).into(),
            cpu_text: app.cpu_pct.map_or("—".into(), format::percent).into(),
            memory_text: format::memory(app.private_bytes).into(),
            icon: icon.cloned().unwrap_or_default(),
            has_icon: icon.is_some(),
            initial: app
                .name
                .chars()
                .find(|c| c.is_alphanumeric())
                .map(|c| c.to_uppercase().collect::<String>())
                .unwrap_or_default()
                .into(),
            armed: self.confirm.armed() == Some(app.key.as_str()),
            ending: self.ending.contains_key(&app.key),
            message: self
                .messages
                .get(&app.key)
                .map(|(m, _)| m.as_str())
                .unwrap_or("")
                .into(),
        }
    }
}

/// How long to wait before calling `expire` after an End click.
pub fn expire_after() -> Duration {
    ARM_TIME.max(ROW_FEEDBACK)
}
