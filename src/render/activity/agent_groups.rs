//! Group `AgentSummary`s into rendering classes by Anthropic `message.id`.
//! See `designs/activity-width-budget.md` §3.

use crate::types::AgentSummary;

/// One agent display unit. Either a single agent or a parallel batch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentGroup<'a> {
    /// A standalone agent — render with the single-agent format.
    Single(&'a AgentSummary),
    /// ≥2 agents from the same assistant turn (`message.id`) sharing
    /// the same `agent_type`. Renders as one collapsed line:
    /// `🤖 type ×N parallel (avg …): first description + N-1 more`.
    Homogeneous(Vec<&'a AgentSummary>),
    /// ≥2 agents from the same assistant turn with mixed `agent_type`s.
    /// Renders as one collapsed line:
    /// `‖ ×N parallel (avg …): type1: desc1 + type2: desc2 + …`.
    Heterogeneous(Vec<&'a AgentSummary>),
}

/// Classify a slice of agents (in original order) into render groups.
///
/// Algorithm:
///   1. Walk in order, partitioning by `message_id`. Agents whose
///      `message_id == None` always become their own `Single`.
///   2. For each `message_id` partition with ≥2 agents:
///      - All same `agent_type` → `Homogeneous`
///      - Mixed `agent_type`    → `Heterogeneous`
///      - (count=1) → `Single`
///   3. Preserve original order: groups appear at the position of their
///      *first* member; later members are absorbed into that group.
pub fn classify(agents: &[AgentSummary]) -> Vec<AgentGroup<'_>> {
    let mut out: Vec<AgentGroup<'_>> = Vec::with_capacity(agents.len());
    let mut consumed = vec![false; agents.len()];

    for (i, anchor) in agents.iter().enumerate() {
        if consumed[i] {
            continue;
        }
        consumed[i] = true;
        let mid = match &anchor.message_id {
            Some(m) => m,
            // No message_id → always Single (safe degradation for legacy
            // cache files).
            None => {
                out.push(AgentGroup::Single(anchor));
                continue;
            }
        };

        // Gather all later agents sharing this message_id.
        let mut group: Vec<&AgentSummary> = vec![anchor];
        for (j, other) in agents.iter().enumerate().skip(i + 1) {
            if consumed[j] {
                continue;
            }
            if other.message_id.as_deref() == Some(mid) {
                consumed[j] = true;
                group.push(other);
            }
        }

        if group.len() == 1 {
            out.push(AgentGroup::Single(anchor));
            continue;
        }
        let same_type = group.iter().all(|a| a.agent_type == anchor.agent_type);
        if same_type {
            out.push(AgentGroup::Homogeneous(group));
        } else {
            out.push(AgentGroup::Heterogeneous(group));
        }
    }

    out
}

/// Average of `started_at`→`completed_at` durations across a group, in
/// milliseconds. Skips agents without both timestamps. Returns `None` if
/// the group has no completed agents with timing data — caller can fall
/// back to elapsed-since-started for the running case.
pub fn avg_elapsed_ms(group: &[&AgentSummary]) -> Option<u64> {
    let mut sum = 0u64;
    let mut n = 0u64;
    for a in group {
        if let (Some(start), Some(end)) = (a.started_at, a.completed_at) {
            sum = sum.saturating_add(end.saturating_sub(start));
            n += 1;
        }
    }
    sum.checked_div(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn agent(id: &str, msg: Option<&str>, ty: Option<&str>) -> AgentSummary {
        AgentSummary {
            id: id.to_string(),
            description: format!("{id} description"),
            agent_type: ty.map(String::from),
            started_at: Some(1000),
            model: None,
            completed_at: Some(60_000),
            message_id: msg.map(String::from),
            agent_id: None,
        }
    }

    #[test]
    fn empty_input_returns_empty() {
        assert!(classify(&[]).is_empty());
    }

    #[test]
    fn single_agent_no_message_id_is_single() {
        let agents = vec![agent("a1", None, Some("Explore"))];
        let groups = classify(&agents);
        assert_eq!(groups.len(), 1);
        assert!(matches!(groups[0], AgentGroup::Single(_)));
    }

    #[test]
    fn missing_message_id_always_single_even_with_neighbors() {
        let agents = vec![
            agent("a1", None, Some("Explore")),
            agent("a2", None, Some("Explore")),
            agent("a3", None, Some("Explore")),
        ];
        let groups = classify(&agents);
        assert_eq!(groups.len(), 3);
        assert!(groups.iter().all(|g| matches!(g, AgentGroup::Single(_))));
    }

    #[test]
    fn same_message_id_same_type_is_homogeneous() {
        let agents = vec![
            agent("a1", Some("msg_X"), Some("general-purpose")),
            agent("a2", Some("msg_X"), Some("general-purpose")),
            agent("a3", Some("msg_X"), Some("general-purpose")),
        ];
        let groups = classify(&agents);
        assert_eq!(groups.len(), 1);
        match &groups[0] {
            AgentGroup::Homogeneous(g) => assert_eq!(g.len(), 3),
            other => panic!("expected Homogeneous, got {other:?}"),
        }
    }

    #[test]
    fn same_message_id_mixed_type_is_heterogeneous() {
        let agents = vec![
            agent("a1", Some("msg_X"), Some("Explore")),
            agent("a2", Some("msg_X"), Some("general-purpose")),
            agent("a3", Some("msg_X"), Some("code-reviewer")),
        ];
        let groups = classify(&agents);
        assert_eq!(groups.len(), 1);
        match &groups[0] {
            AgentGroup::Heterogeneous(g) => assert_eq!(g.len(), 3),
            other => panic!("expected Heterogeneous, got {other:?}"),
        }
    }

    #[test]
    fn different_message_ids_stay_sequential() {
        let agents = vec![
            agent("a1", Some("msg_A"), Some("general-purpose")),
            agent("a2", Some("msg_B"), Some("general-purpose")),
            agent("a3", Some("msg_C"), Some("general-purpose")),
        ];
        let groups = classify(&agents);
        assert_eq!(groups.len(), 3);
        assert!(groups.iter().all(|g| matches!(g, AgentGroup::Single(_))));
    }

    #[test]
    fn interleaved_batches_preserve_order() {
        // a1 (msg_A) - a2 (msg_B) - a3 (msg_A) - a4 (msg_B) - a5 (msg_C)
        // Should yield: [Homogeneous(a1, a3), Homogeneous(a2, a4), Single(a5)]
        let agents = vec![
            agent("a1", Some("msg_A"), Some("Explore")),
            agent("a2", Some("msg_B"), Some("general-purpose")),
            agent("a3", Some("msg_A"), Some("Explore")),
            agent("a4", Some("msg_B"), Some("general-purpose")),
            agent("a5", Some("msg_C"), Some("Explore")),
        ];
        let groups = classify(&agents);
        assert_eq!(groups.len(), 3);
        match &groups[0] {
            AgentGroup::Homogeneous(g) => {
                assert_eq!(g[0].id, "a1");
                assert_eq!(g[1].id, "a3");
            }
            other => panic!("expected Homogeneous([a1,a3]), got {other:?}"),
        }
        match &groups[1] {
            AgentGroup::Homogeneous(g) => {
                assert_eq!(g[0].id, "a2");
                assert_eq!(g[1].id, "a4");
            }
            other => panic!("expected Homogeneous([a2,a4]), got {other:?}"),
        }
        assert!(matches!(groups[2], AgentGroup::Single(a) if a.id == "a5"));
    }

    #[test]
    fn lone_agent_with_message_id_is_single() {
        // message_id present but only one agent shares it → Single.
        let agents = vec![agent("a1", Some("msg_X"), Some("Explore"))];
        let groups = classify(&agents);
        assert_eq!(groups.len(), 1);
        assert!(matches!(groups[0], AgentGroup::Single(_)));
    }

    #[test]
    fn avg_elapsed_averages_completed() {
        let mut a1 = agent("a1", None, None);
        a1.started_at = Some(1000);
        a1.completed_at = Some(61_000); // 60s
        let mut a2 = agent("a2", None, None);
        a2.started_at = Some(2000);
        a2.completed_at = Some(122_000); // 120s
        let group = vec![&a1, &a2];
        assert_eq!(avg_elapsed_ms(&group), Some(90_000)); // (60+120)/2 = 90s
    }

    #[test]
    fn avg_elapsed_skips_running() {
        let mut completed = agent("a1", None, None);
        completed.started_at = Some(1000);
        completed.completed_at = Some(61_000);
        let mut running = agent("a2", None, None);
        running.started_at = Some(2000);
        running.completed_at = None;
        let group = vec![&completed, &running];
        assert_eq!(avg_elapsed_ms(&group), Some(60_000)); // only the completed one
    }

    #[test]
    fn avg_elapsed_none_when_no_timing() {
        let mut a1 = agent("a1", None, None);
        a1.started_at = None;
        a1.completed_at = None;
        let group = vec![&a1];
        assert_eq!(avg_elapsed_ms(&group), None);
    }
}
