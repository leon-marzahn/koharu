use std::collections::{BTreeMap, BTreeSet};

use koharu_scene::EntityId;

use crate::Stage;

#[derive(Clone, Copy, Eq, PartialEq)]
enum WorkState {
    Pending,
    Running,
    Finished,
}

struct StageWork {
    stage: Stage,
    state: WorkState,
}

struct PageWork {
    page: EntityId,
    stages: Vec<StageWork>,
}

impl PageWork {
    fn started(&self) -> bool {
        self.stages
            .iter()
            .any(|work| work.state != WorkState::Pending)
    }

    fn finished(&self) -> bool {
        self.stages
            .iter()
            .all(|work| work.state == WorkState::Finished)
    }

    fn ready(&self, index: usize) -> bool {
        // A prerequisite outside the run does not block. A skipped one is marked Finished by the
        // executor; a failed one aborts the whole run, so nothing waits on it.
        prerequisites(self.stages[index].stage).iter().all(|need| {
            self.stages
                .iter()
                .find(|work| work.stage == *need)
                .is_none_or(|work| work.state == WorkState::Finished)
        })
    }
}

pub(crate) struct Scheduler {
    pages: Vec<PageWork>,
    page_index: BTreeMap<EntityId, usize>,
    page_window: usize,
    active_pages: usize,
    head: usize,
    total: usize,
}

impl Scheduler {
    pub(crate) fn new(pages: &[EntityId], stages: &[Stage]) -> Self {
        let pages = pages
            .iter()
            .map(|page| PageWork {
                page: *page,
                stages: stages
                    .iter()
                    .map(|stage| StageWork {
                        stage: *stage,
                        state: WorkState::Pending,
                    })
                    .collect(),
            })
            .collect::<Vec<_>>();
        let total = pages.len().saturating_mul(stages.len());
        Self {
            page_index: pages
                .iter()
                .enumerate()
                .map(|(index, page)| (page.page, index))
                .collect(),
            pages,
            page_window: stages.len().max(1),
            active_pages: 0,
            head: 0,
            total,
        }
    }

    pub(crate) fn total(&self) -> usize {
        self.total
    }

    pub(crate) fn start_next(
        &mut self,
        busy_stages: &BTreeSet<Stage>,
    ) -> Option<(EntityId, Stage)> {
        for page_index in self.head..self.pages.len() {
            let started = self.pages[page_index].started();
            if !started && self.active_pages >= self.page_window {
                break;
            }
            let stage_index =
                self.pages[page_index]
                    .stages
                    .iter()
                    .enumerate()
                    .find_map(|(index, work)| {
                        (work.state == WorkState::Pending
                            && !busy_stages.contains(&work.stage)
                            && self.pages[page_index].ready(index))
                        .then_some(index)
                    });
            let Some(stage_index) = stage_index else {
                continue;
            };
            if !started {
                self.active_pages += 1;
            }
            let page = &mut self.pages[page_index];
            let work = &mut page.stages[stage_index];
            work.state = WorkState::Running;
            return Some((page.page, work.stage));
        }
        None
    }

    pub(crate) fn complete_stage(&mut self, page: EntityId, stage: Stage) -> bool {
        let Some(&page_index) = self.page_index.get(&page) else {
            return false;
        };
        let page = &mut self.pages[page_index];
        let was_finished = page.finished();
        if let Some(work) = page.stages.iter_mut().find(|work| work.stage == stage) {
            work.state = WorkState::Finished;
        }
        let page_finished = !was_finished && page.finished();
        if page_finished {
            self.active_pages = self.active_pages.saturating_sub(1);
            while self.head < self.pages.len() && self.pages[self.head].finished() {
                self.head += 1;
            }
        }
        page_finished
    }
}

/// Translation also waits for Inpainting so the description sees the cleaned page.
const fn prerequisites(stage: Stage) -> &'static [Stage] {
    match stage {
        Stage::Detection => &[],
        Stage::Ocr | Stage::Inpainting => &[Stage::Detection],
        Stage::Translation => &[Stage::Ocr, Stage::Inpainting],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pages(count: usize) -> Vec<EntityId> {
        (0..count).map(|_| EntityId::new()).collect()
    }

    #[test]
    fn starts_pages_in_order_and_models_independently() {
        let pages = pages(2);
        let mut scheduler = Scheduler::new(&pages, &Stage::ALL);
        let mut busy = BTreeSet::new();

        let first = scheduler.start_next(&busy).unwrap();
        assert_eq!(first, (pages[0], Stage::Detection));
        busy.insert(Stage::Detection);
        assert!(scheduler.start_next(&busy).is_none());

        busy.clear();
        assert!(!scheduler.complete_stage(pages[0], Stage::Detection));
        let ocr = scheduler.start_next(&busy).unwrap();
        busy.insert(ocr.1);
        let inpainting = scheduler.start_next(&busy).unwrap();
        busy.insert(inpainting.1);
        let next_page = scheduler.start_next(&busy).unwrap();
        busy.insert(next_page.1);
        assert_eq!(ocr, (pages[0], Stage::Ocr));
        assert_eq!(inpainting, (pages[0], Stage::Inpainting));
        assert_eq!(next_page, (pages[1], Stage::Detection));

        assert!(!scheduler.complete_stage(pages[0], Stage::Ocr));
        busy.remove(&Stage::Ocr);
        // Translation waits for Inpainting; the next page's Detection is already running.
        assert!(scheduler.start_next(&busy).is_none());
        assert!(!scheduler.complete_stage(pages[0], Stage::Inpainting));
        busy.remove(&Stage::Inpainting);
        let translation = scheduler.start_next(&busy).unwrap();
        assert_eq!(translation, (pages[0], Stage::Translation));
        assert!(busy.contains(&Stage::Detection));
    }

    #[test]
    fn sliding_window_backpressures_fast_upstream_models() {
        let pages = pages(4);
        let stages = [Stage::Detection, Stage::Ocr, Stage::Inpainting];
        let mut scheduler = Scheduler::new(&pages, &stages);
        let mut busy = BTreeSet::new();

        assert_eq!(
            scheduler.start_next(&busy),
            Some((pages[0], Stage::Detection))
        );
        assert!(!scheduler.complete_stage(pages[0], Stage::Detection));
        let ocr = scheduler.start_next(&busy).unwrap();
        busy.insert(ocr.1);
        let inpainting = scheduler.start_next(&busy).unwrap();
        busy.insert(inpainting.1);

        for page in &pages[1..3] {
            assert_eq!(scheduler.start_next(&busy), Some((*page, Stage::Detection)));
            assert!(!scheduler.complete_stage(*page, Stage::Detection));
        }
        assert!(scheduler.start_next(&busy).is_none());

        assert!(!scheduler.complete_stage(pages[0], Stage::Ocr));
        assert!(scheduler.complete_stage(pages[0], Stage::Inpainting));
        busy.clear();
        assert_eq!(scheduler.start_next(&busy), Some((pages[1], Stage::Ocr)));
    }

    fn run_order(stages: &[Stage]) -> Vec<Stage> {
        let pages = pages(1);
        let mut scheduler = Scheduler::new(&pages, stages);
        let mut order = Vec::new();
        while let Some((page, stage)) = scheduler.start_next(&BTreeSet::new()) {
            order.push(stage);
            scheduler.complete_stage(page, stage);
        }
        order
    }

    #[test]
    fn full_run_translates_after_inpainting() {
        let order = run_order(&Stage::ALL);
        let at = |stage| order.iter().position(|s| *s == stage).unwrap();
        assert!(at(Stage::Inpainting) < at(Stage::Translation));
        assert!(at(Stage::Ocr) < at(Stage::Translation));
    }

    #[test]
    fn translation_waits_for_ocr_without_inpainting() {
        let pages = pages(1);
        let mut scheduler = Scheduler::new(&pages, &[Stage::Translation, Stage::Ocr]);
        let busy = BTreeSet::new();
        assert_eq!(scheduler.start_next(&busy).unwrap().1, Stage::Ocr);
        assert!(scheduler.start_next(&busy).is_none());
    }

    #[test]
    fn translation_alone_and_skipped_inpainting_do_not_block() {
        assert_eq!(run_order(&[Stage::Translation]), [Stage::Translation]);
        // The executor marks a skipped stage complete, which releases Translation.
        let pages = pages(1);
        let mut scheduler = Scheduler::new(&pages, &[Stage::Inpainting, Stage::Translation]);
        let busy = BTreeSet::new();
        assert_eq!(scheduler.start_next(&busy).unwrap().1, Stage::Inpainting);
        assert!(scheduler.start_next(&busy).is_none());
        scheduler.complete_stage(pages[0], Stage::Inpainting);
        assert_eq!(scheduler.start_next(&busy).unwrap().1, Stage::Translation);
    }
}
