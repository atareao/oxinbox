use std::collections::HashMap;

use oxinbox_core::{Task, Uuid};
use serde::{Deserialize, Serialize};
use tracing::instrument;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub task: Task,
    pub score: f64,
}

fn bm25_text(task: &Task) -> String {
    let mut parts = vec![task.description.clone()];
    for p in &task.projects {
        parts.push(format!("+{p}"));
    }
    for c in &task.contexts {
        parts.push(format!("@{c}"));
    }
    if let Some(p) = task.priority {
        parts.push(format!("({p})"));
    }
    parts.join(" ")
}

#[derive(Default)]
pub struct SearchIndex {
    pub bm25: Bm25Index,
    vectors: HashMap<Uuid, Vec<f32>>,
}

impl SearchIndex {
    pub fn index_task(&mut self, task: &Task) {
        self.bm25.index(task.id, &bm25_text(task));
    }

    pub fn remove_task(&mut self, id: Uuid) {
        self.bm25.remove(id);
        self.vectors.remove(&id);
    }

    pub fn store_embedding(&mut self, task_id: Uuid, embedding: Vec<f32>) {
        self.vectors.insert(task_id, embedding);
    }
}

#[derive(Default)]
pub struct Bm25Index {
    doc_count: usize,
    doc_freq: HashMap<String, usize>,
    term_counts: HashMap<Uuid, HashMap<String, usize>>,
    doc_lengths: HashMap<Uuid, usize>,
}

impl Bm25Index {
    const K1: f64 = 1.5;
    const B: f64 = 0.75;

    fn tokenize(text: &str) -> Vec<String> {
        text.to_lowercase()
            .split_whitespace()
            .filter(|t| t.len() >= 2)
            .map(|t| {
                t.trim_matches(|c: char| c.is_ascii_punctuation())
                    .to_string()
            })
            .filter(|t| !t.is_empty())
            .collect()
    }

    fn index(&mut self, id: Uuid, text: &str) {
        self.remove(id);

        let tokens = Self::tokenize(text);
        let len = tokens.len();

        let mut counts: HashMap<String, usize> = HashMap::new();
        for token in &tokens {
            *counts.entry(token.clone()).or_default() += 1;
        }

        for token in counts.keys() {
            *self.doc_freq.entry(token.clone()).or_default() += 1;
        }

        self.term_counts.insert(id, counts);
        self.doc_lengths.insert(id, len);
        self.doc_count += 1;
    }

    fn remove(&mut self, id: Uuid) {
        if let Some(counts) = self.term_counts.remove(&id) {
            for token in counts.keys() {
                if let Some(freq) = self.doc_freq.get_mut(token) {
                    *freq = freq.saturating_sub(1);
                    if *freq == 0 {
                        self.doc_freq.remove(token);
                    }
                }
            }
            self.doc_lengths.remove(&id);
            self.doc_count = self.doc_count.saturating_sub(1);
        }
    }

    #[allow(
        clippy::cast_precision_loss,
        clippy::imprecise_flops,
        clippy::suboptimal_flops
    )]
    fn score(&self, id: Uuid, query_tokens: &[String]) -> f64 {
        let Some(counts) = self.term_counts.get(&id) else {
            return 0.0;
        };
        let Some(&doc_len) = self.doc_lengths.get(&id) else {
            return 0.0;
        };
        let avg_dl =
            self.doc_lengths.values().copied().sum::<usize>() as f64 / self.doc_count.max(1) as f64;

        let mut score = 0.0;
        for token in query_tokens {
            let tf = *counts.get(token).unwrap_or(&0) as f64;
            let df = *self.doc_freq.get(token).unwrap_or(&1) as f64;
            let idf = ((self.doc_count as f64 - df + 0.5) / (df + 0.5)).ln_1p();
            let numerator = tf * (Self::K1 + 1.0);
            let denominator =
                Self::K1.mul_add(1.0 - Self::B + Self::B * doc_len as f64 / avg_dl, tf);
            score += idf * numerator / denominator;
        }
        score
    }
}

pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f64 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    f64::from(dot / (norm_a * norm_b))
}

#[instrument(skip(index))]
pub fn hybrid_search(
    tasks: &[Task],
    index: &SearchIndex,
    query_text: &str,
    query_embedding: Option<&[f32]>,
    limit: usize,
    alpha: f64,
) -> Vec<SearchResult> {
    let query_tokens = Bm25Index::tokenize(query_text);

    let bm25_scores: HashMap<Uuid, f64> = tasks
        .iter()
        .map(|t| (t.id, index.bm25.score(t.id, &query_tokens)))
        .collect();

    let bm25_max = bm25_scores.values().copied().fold(0.0_f64, f64::max);
    let bm25_min = bm25_scores.values().copied().fold(f64::MAX, f64::min);

    let vector_scores: HashMap<Uuid, f64> = query_embedding.map_or_else(HashMap::new, |qv| {
        tasks
            .iter()
            .map(|t| {
                let sim = index
                    .vectors
                    .get(&t.id)
                    .map_or(0.0, |ev| cosine_similarity(qv, ev));
                (t.id, sim)
            })
            .collect()
    });

    let vec_max = vector_scores.values().copied().fold(0.0_f64, f64::max);
    let vec_min = vector_scores.values().copied().fold(f64::MAX, f64::min);

    let mut results: Vec<SearchResult> = tasks
        .iter()
        .map(|task| {
            let bm25_norm = if bm25_max > bm25_min {
                (bm25_scores[&task.id] - bm25_min) / (bm25_max - bm25_min)
            } else {
                bm25_scores[&task.id]
            };

            let vec_norm = if vec_max > vec_min {
                (vector_scores.get(&task.id).copied().unwrap_or(0.0) - vec_min)
                    / (vec_max - vec_min)
            } else {
                vector_scores.get(&task.id).copied().unwrap_or(0.0)
            };

            let score = alpha.mul_add(bm25_norm, (1.0 - alpha) * vec_norm);

            SearchResult {
                task: task.clone(),
                score,
            }
        })
        .filter(|r| r.score > 0.0)
        .collect();

    results.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    results.truncate(limit);
    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn make_task(id: Uuid, desc: &str, projects: &[&str], contexts: &[&str]) -> Task {
        Task {
            id,
            completed: false,
            priority: None,
            description: desc.into(),
            projects: projects.iter().map(ToString::to_string).collect(),
            contexts: contexts.iter().map(ToString::to_string).collect(),
            status: oxinbox_core::TaskStatus::Inbox,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            completed_at: None,
            due_date: None,
        }
    }

    #[test]
    fn bm25_ranks_relevant_higher() {
        let t1 = make_task(Uuid::now_v7(), "buy milk from the store", &[], &[]);
        let t2 = make_task(Uuid::now_v7(), "fix database indexing bug", &[], &[]);

        let mut idx = Bm25Index::default();
        idx.index(t1.id, &bm25_text(&t1));
        idx.index(t2.id, &bm25_text(&t2));

        let tokens = Bm25Index::tokenize("milk store");
        let s1 = idx.score(t1.id, &tokens);
        let s2 = idx.score(t2.id, &tokens);

        assert!(
            s1 > s2,
            "BM25 should rank 'milk store' higher for the milk task"
        );
    }

    #[test]
    fn hybrid_search_returns_results() {
        let t1 = make_task(Uuid::now_v7(), "buy milk", &["proyecto"], &["casa"]);
        let t2 = make_task(Uuid::now_v7(), "fix database", &[], &[]);
        let tasks = vec![t1.clone(), t2.clone()];

        let mut bm25 = Bm25Index::default();
        bm25.index(t1.id, &bm25_text(&t1));
        bm25.index(t2.id, &bm25_text(&t2));

        let mut vs = SearchIndex::default();
        vs.index_task(&t1);
        vs.index_task(&t2);

        let results = hybrid_search(&tasks, &vs, "milk", None, 10, 1.0);
        assert!(!results.is_empty());
        assert_eq!(results[0].task.description, "buy milk");
    }

    #[test]
    fn cosine_similarity_identical() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![1.0, 2.0, 3.0];
        let sim = cosine_similarity(&a, &b);
        assert!((sim - 1.0).abs() < 1e-6);
    }

    #[test]
    fn cosine_similarity_orthogonal() {
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];
        let sim = cosine_similarity(&a, &b);
        assert!((sim - 0.0).abs() < 1e-6);
    }
}
