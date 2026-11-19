//! Diplomacy between nations (gameplay specification §8): relation states,
//! unilateral actions, bilateral proposals answered one turn later, defensive
//! alliances, separate peace with a truce, the warning period and reputation.
//!
//! Every pair of nations has one relation, `Neutral` until something changes
//! it. The rules are checked here against the current state; World adds the
//! checks that need the rest of the game (whether the nations exist).
use crate::domain::ids::NationId;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The relation between two nations (§8.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum Relation {
    /// The default: trade with a high tariff.
    Neutral,
    /// No trade, no war. Only the imposing nation can lift it.
    Embargo { imposed_by: NationId },
    /// Trade with a low tariff.
    TradeAgreement,
    /// No tariff and passage for armies; defensive (§8.3).
    Alliance,
    /// Armies may enter and fight; no trade. `aggressor` declared the war
    /// (or attacked the ally that this nation defends).
    War { aggressor: NationId },
    /// After peace: no war for `turns_left` more turns; trade as in neutrality.
    Truce { turns_left: u32 },
}

impl Relation {
    /// Settlements of the two nations may trade (§8.1, §9).
    pub fn allows_trade(self) -> bool {
        !matches!(self, Self::War { .. } | Self::Embargo { .. })
    }

    /// Customs duty between the two nations (§9), for the customs system;
    /// None when trade is blocked. Decrees change it by `tariff_multiplier`.
    pub fn tariff_rate(self) -> Option<f32> {
        match self {
            Self::Neutral | Self::Truce { .. } => Some(0.2),
            Self::TradeAgreement => Some(0.05),
            Self::Alliance => Some(0.0),
            Self::War { .. } | Self::Embargo { .. } => None,
        }
    }
}

/// What a bilateral proposal offers (§8.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Treaty {
    /// From neutrality.
    TradeAgreement,
    /// From neutrality or a trade agreement.
    Alliance,
    /// From war; starts a truce.
    Peace,
}

impl Treaty {
    fn can_start_from(self, relation: Relation) -> bool {
        match self {
            Self::TradeAgreement => relation == Relation::Neutral,
            Self::Alliance => matches!(relation, Relation::Neutral | Relation::TradeAgreement),
            Self::Peace => matches!(relation, Relation::War { .. }),
        }
    }

    fn resulting_relation(self) -> Relation {
        match self {
            Self::TradeAgreement => Relation::TradeAgreement,
            Self::Alliance => Relation::Alliance,
            Self::Peace => Relation::Truce {
                turns_left: Diplomacy::TRUCE_TURNS,
            },
        }
    }
}

/// One diplomatic action of a nation (§8.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiplomaticAction {
    DeclareWar {
        target: NationId,
    },
    DeclareEmbargo {
        target: NationId,
    },
    LiftEmbargo {
        target: NationId,
    },
    BreakTradeAgreement {
        target: NationId,
    },
    BreakAlliance {
        target: NationId,
    },
    Propose {
        target: NationId,
        treaty: Treaty,
    },
    Respond {
        proposer: NationId,
        treaty: Treaty,
        accept: bool,
    },
}

impl DiplomaticAction {
    /// The other nation of the action.
    pub fn other(self) -> NationId {
        match self {
            Self::DeclareWar { target }
            | Self::DeclareEmbargo { target }
            | Self::LiftEmbargo { target }
            | Self::BreakTradeAgreement { target }
            | Self::BreakAlliance { target }
            | Self::Propose { target, .. } => target,
            Self::Respond { proposer, .. } => proposer,
        }
    }
}

/// Why a diplomatic action is not possible now.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "reason", rename_all = "snake_case", deny_unknown_fields)]
pub enum DiplomacyError {
    UnknownNation {
        nation_id: NationId,
    },
    DiplomacyWithSelf,
    /// The action does not start from the current relation (§8.2 table).
    InvalidRelation {
        nation_id: NationId,
        relation: Relation,
    },
    /// No war during a truce.
    TruceInForce {
        nation_id: NationId,
        turns_left: u32,
    },
    /// A treaty with this nation was broken this turn: war is possible from
    /// the next turn (the warning period).
    WarningPeriod {
        nation_id: NationId,
    },
    /// Only the nation that imposed an embargo can lift it.
    NotEmbargoImposer {
        nation_id: NationId,
    },
    /// The same proposal to this nation still waits for an answer.
    ProposalAlreadyPending {
        nation_id: NationId,
        treaty: Treaty,
    },
    /// No proposal of this kind from this nation can be answered now (none,
    /// made this turn, or expired).
    NoSuchProposal {
        nation_id: NationId,
        treaty: Treaty,
    },
}

/// What an applied diplomatic action did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "effect", rename_all = "snake_case", deny_unknown_fields)]
pub enum DiplomaticOutcome {
    /// `allies_joined`: allies of the target that entered the war (§8.3).
    WarDeclared {
        target: NationId,
        allies_joined: Vec<NationId>,
    },
    EmbargoDeclared {
        target: NationId,
    },
    EmbargoLifted {
        target: NationId,
    },
    TreatyBroken {
        target: NationId,
        treaty: Treaty,
    },
    ProposalSent {
        target: NationId,
        treaty: Treaty,
    },
    ProposalAccepted {
        proposer: NationId,
        treaty: Treaty,
        relation: Relation,
    },
    ProposalRejected {
        proposer: NationId,
        treaty: Treaty,
    },
}

/// Something that happened to a nation, for its turn report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case", deny_unknown_fields)]
pub enum DiplomaticEvent {
    /// `aggressor` declared war on this nation.
    WarDeclared {
        aggressor: NationId,
    },
    /// For an ally: this nation entered the war against `aggressor` to defend
    /// `ally`. For the aggressor: `ally` of its target entered the war.
    DefensiveWarJoined {
        aggressor: NationId,
        ally: NationId,
    },
    EmbargoDeclared {
        by: NationId,
    },
    EmbargoLifted {
        by: NationId,
    },
    TreatyBroken {
        by: NationId,
        treaty: Treaty,
    },
    ProposalReceived {
        from: NationId,
        treaty: Treaty,
    },
    ProposalAccepted {
        by: NationId,
        treaty: Treaty,
    },
    ProposalRejected {
        by: NationId,
        treaty: Treaty,
    },
    /// A proposal between `from` and `to` was not answered in time.
    ProposalExpired {
        from: NationId,
        to: NationId,
        treaty: Treaty,
    },
    TruceEnded {
        with: NationId,
    },
}

/// A proposal waiting for its answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Proposal {
    pub from: NationId,
    pub to: NationId,
    pub treaty: Treaty,
    /// False in the turn it was made; the target answers in the next turn.
    pub answerable: bool,
}

/// Per-pair state besides the relation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Pair {
    relation: Relation,
    /// A treaty of this pair was broken this turn: no war until the next turn.
    warning: bool,
    /// The last treaty break of this pair: who broke it and how many turns ago.
    last_break: Option<(NationId, u32)>,
}

impl Default for Pair {
    fn default() -> Self {
        Self {
            relation: Relation::Neutral,
            warning: false,
            last_break: None,
        }
    }
}

/// All diplomatic state of the game.
#[derive(Debug, Default)]
pub struct Diplomacy {
    /// Keyed by the ordered pair (lower ID, higher ID); missing pairs are neutral.
    pairs: BTreeMap<(NationId, NationId), Pair>,
    proposals: Vec<Proposal>,
    /// Broken treaties per nation (§8.4); public.
    reputation: BTreeMap<NationId, u32>,
    /// Events not yet delivered to the nations' turn reports.
    events: Vec<(NationId, DiplomaticEvent)>,
}

fn key(a: NationId, b: NationId) -> (NationId, NationId) {
    (a.min(b), a.max(b))
}

impl Diplomacy {
    /// Length of the truce after peace.
    pub const TRUCE_TURNS: u32 = 10;
    /// A war declared within this many turns after breaking a treaty with the
    /// same nation counts as another broken treaty.
    pub const BETRAYAL_WINDOW_TURNS: u32 = 5;

    /// The relation of two different nations.
    pub fn relation(&self, a: NationId, b: NationId) -> Relation {
        self.pairs
            .get(&key(a, b))
            .map_or(Relation::Neutral, |pair| pair.relation)
    }

    /// A treaty between the two was broken this turn: war waits a turn.
    pub fn in_warning_period(&self, a: NationId, b: NationId) -> bool {
        self.pairs.get(&key(a, b)).is_some_and(|pair| pair.warning)
    }

    /// Broken treaties of a nation (zero when none).
    pub fn reputation(&self, nation: NationId) -> u32 {
        self.reputation.get(&nation).copied().unwrap_or(0)
    }

    pub fn proposals(&self) -> &[Proposal] {
        &self.proposals
    }

    fn pair_mut(&mut self, a: NationId, b: NationId) -> &mut Pair {
        self.pairs.entry(key(a, b)).or_default()
    }

    fn set_relation(&mut self, a: NationId, b: NationId, relation: Relation) {
        self.pair_mut(a, b).relation = relation;
    }

    fn proposal_index(&self, from: NationId, to: NationId, treaty: Treaty) -> Option<usize> {
        self.proposals
            .iter()
            .position(|p| p.from == from && p.to == to && p.treaty == treaty)
    }

    fn invalid(&self, actor: NationId, other: NationId) -> DiplomacyError {
        DiplomacyError::InvalidRelation {
            nation_id: other,
            relation: self.relation(actor, other),
        }
    }

    /// Whether `actor` may take the action now. Does not check that nations exist.
    pub fn check(&self, actor: NationId, action: DiplomaticAction) -> Result<(), DiplomacyError> {
        let other = action.other();
        if other == actor {
            return Err(DiplomacyError::DiplomacyWithSelf);
        }
        let relation = self.relation(actor, other);
        match action {
            DiplomaticAction::DeclareWar { .. } => match relation {
                Relation::Truce { turns_left } => {
                    return Err(DiplomacyError::TruceInForce {
                        nation_id: other,
                        turns_left,
                    });
                }
                Relation::Neutral | Relation::Embargo { .. } => {
                    if self.in_warning_period(actor, other) {
                        return Err(DiplomacyError::WarningPeriod { nation_id: other });
                    }
                }
                _ => return Err(self.invalid(actor, other)),
            },
            DiplomaticAction::DeclareEmbargo { .. } => {
                if relation != Relation::Neutral {
                    return Err(self.invalid(actor, other));
                }
            }
            DiplomaticAction::LiftEmbargo { .. } => match relation {
                Relation::Embargo { imposed_by } if imposed_by == actor => {}
                Relation::Embargo { .. } => {
                    return Err(DiplomacyError::NotEmbargoImposer { nation_id: other });
                }
                _ => return Err(self.invalid(actor, other)),
            },
            DiplomaticAction::BreakTradeAgreement { .. } => {
                if relation != Relation::TradeAgreement {
                    return Err(self.invalid(actor, other));
                }
            }
            DiplomaticAction::BreakAlliance { .. } => {
                if relation != Relation::Alliance {
                    return Err(self.invalid(actor, other));
                }
            }
            DiplomaticAction::Propose { treaty, .. } => {
                if !treaty.can_start_from(relation) {
                    return Err(self.invalid(actor, other));
                }
                if self.proposal_index(actor, other, treaty).is_some() {
                    return Err(DiplomacyError::ProposalAlreadyPending {
                        nation_id: other,
                        treaty,
                    });
                }
            }
            DiplomaticAction::Respond { treaty, accept, .. } => {
                let answerable = self
                    .proposal_index(other, actor, treaty)
                    .is_some_and(|index| self.proposals[index].answerable);
                if !answerable {
                    return Err(DiplomacyError::NoSuchProposal {
                        nation_id: other,
                        treaty,
                    });
                }
                // Acceptance checks the transition again: the relation may have
                // changed since the proposal was made.
                if accept && !treaty.can_start_from(relation) {
                    return Err(self.invalid(actor, other));
                }
            }
        }
        Ok(())
    }

    /// Checks and applies the action. `nations` lists every nation in the
    /// game, for the defensive alliance.
    pub(crate) fn apply(
        &mut self,
        actor: NationId,
        action: DiplomaticAction,
        nations: &[NationId],
    ) -> Result<DiplomaticOutcome, DiplomacyError> {
        self.check(actor, action)?;
        Ok(match action {
            DiplomaticAction::DeclareWar { target } => {
                // §8.4: a war soon after breaking a treaty with the same nation
                // counts as another broken treaty.
                let last_break = self
                    .pairs
                    .get(&key(actor, target))
                    .and_then(|pair| pair.last_break);
                if let Some((breaker, turns_ago)) = last_break
                    && breaker == actor
                    && turns_ago <= Self::BETRAYAL_WINDOW_TURNS
                {
                    *self.reputation.entry(actor).or_default() += 1;
                }
                self.set_relation(actor, target, Relation::War { aggressor: actor });
                self.events
                    .push((target, DiplomaticEvent::WarDeclared { aggressor: actor }));
                let allies_joined = self.defend_allies(actor, target, nations);
                DiplomaticOutcome::WarDeclared {
                    target,
                    allies_joined,
                }
            }
            DiplomaticAction::DeclareEmbargo { target } => {
                self.set_relation(actor, target, Relation::Embargo { imposed_by: actor });
                self.events
                    .push((target, DiplomaticEvent::EmbargoDeclared { by: actor }));
                DiplomaticOutcome::EmbargoDeclared { target }
            }
            DiplomaticAction::LiftEmbargo { target } => {
                self.set_relation(actor, target, Relation::Neutral);
                self.events
                    .push((target, DiplomaticEvent::EmbargoLifted { by: actor }));
                DiplomaticOutcome::EmbargoLifted { target }
            }
            DiplomaticAction::BreakTradeAgreement { target } => {
                self.break_treaty(actor, target, Treaty::TradeAgreement)
            }
            DiplomaticAction::BreakAlliance { target } => {
                self.break_treaty(actor, target, Treaty::Alliance)
            }
            DiplomaticAction::Propose { target, treaty } => {
                self.proposals.push(Proposal {
                    from: actor,
                    to: target,
                    treaty,
                    answerable: false,
                });
                self.events.push((
                    target,
                    DiplomaticEvent::ProposalReceived {
                        from: actor,
                        treaty,
                    },
                ));
                DiplomaticOutcome::ProposalSent { target, treaty }
            }
            DiplomaticAction::Respond {
                proposer,
                treaty,
                accept,
            } => {
                let index = self
                    .proposal_index(proposer, actor, treaty)
                    .expect("checked above");
                self.proposals.remove(index);
                if accept {
                    // Peace keeps every settlement with its current owner
                    // (§8.5): it only changes the relation.
                    let relation = treaty.resulting_relation();
                    self.set_relation(actor, proposer, relation);
                    self.events.push((
                        proposer,
                        DiplomaticEvent::ProposalAccepted { by: actor, treaty },
                    ));
                    DiplomaticOutcome::ProposalAccepted {
                        proposer,
                        treaty,
                        relation,
                    }
                } else {
                    self.events.push((
                        proposer,
                        DiplomaticEvent::ProposalRejected { by: actor, treaty },
                    ));
                    DiplomaticOutcome::ProposalRejected { proposer, treaty }
                }
            }
        })
    }

    /// §8.3: every ally of the target enters the war against the aggressor,
    /// except an ally that is also allied with the aggressor. An ally already at
    /// war with the aggressor stays at war; an ally in a truce with it keeps
    /// the truce. Allies of allies are not drawn in.
    fn defend_allies(
        &mut self,
        aggressor: NationId,
        target: NationId,
        nations: &[NationId],
    ) -> Vec<NationId> {
        let mut joined = Vec::new();
        for &ally in nations {
            if ally == aggressor || ally == target {
                continue;
            }
            if self.relation(target, ally) != Relation::Alliance {
                continue;
            }
            match self.relation(aggressor, ally) {
                Relation::Alliance | Relation::War { .. } | Relation::Truce { .. } => continue,
                _ => {}
            }
            self.set_relation(aggressor, ally, Relation::War { aggressor });
            self.events.push((
                ally,
                DiplomaticEvent::DefensiveWarJoined {
                    aggressor,
                    ally: target,
                },
            ));
            self.events.push((
                aggressor,
                DiplomaticEvent::DefensiveWarJoined { aggressor, ally },
            ));
            joined.push(ally);
        }
        joined
    }

    /// Back to neutrality; a warning turn starts and the reputation suffers.
    fn break_treaty(
        &mut self,
        actor: NationId,
        target: NationId,
        treaty: Treaty,
    ) -> DiplomaticOutcome {
        let pair = self.pair_mut(actor, target);
        pair.relation = Relation::Neutral;
        pair.warning = true;
        pair.last_break = Some((actor, 0));
        *self.reputation.entry(actor).or_default() += 1;
        self.events
            .push((target, DiplomaticEvent::TreatyBroken { by: actor, treaty }));
        DiplomaticOutcome::TreatyBroken { target, treaty }
    }

    /// End of a global turn: truces run down (ending in neutrality), warning
    /// periods end, break ages grow, unanswered proposals of the answer turn
    /// expire and this turn's proposals become answerable.
    pub(crate) fn advance_turn(&mut self) {
        let mut ended = Vec::new();
        for (&(a, b), pair) in &mut self.pairs {
            pair.warning = false;
            if let Some((_, turns_ago)) = &mut pair.last_break {
                *turns_ago += 1;
            }
            if pair
                .last_break
                .is_some_and(|(_, turns_ago)| turns_ago > Self::BETRAYAL_WINDOW_TURNS)
            {
                pair.last_break = None;
            }
            if let Relation::Truce { turns_left } = &mut pair.relation {
                *turns_left = turns_left.saturating_sub(1);
                if *turns_left == 0 {
                    pair.relation = Relation::Neutral;
                    ended.push((a, b));
                }
            }
        }
        for (a, b) in ended {
            self.events
                .push((a, DiplomaticEvent::TruceEnded { with: b }));
            self.events
                .push((b, DiplomaticEvent::TruceEnded { with: a }));
        }
        let mut expired = Vec::new();
        self.proposals.retain(|proposal| {
            if proposal.answerable {
                expired.push(*proposal);
                false
            } else {
                true
            }
        });
        for proposal in &mut self.proposals {
            proposal.answerable = true;
        }
        for proposal in expired {
            let event = DiplomaticEvent::ProposalExpired {
                from: proposal.from,
                to: proposal.to,
                treaty: proposal.treaty,
            };
            self.events.push((proposal.from, event.clone()));
            self.events.push((proposal.to, event));
        }
    }

    /// Forgets a nation that left the game: its relations, proposals,
    /// reputation and undelivered events.
    pub(crate) fn remove_nation(&mut self, nation: NationId) {
        self.pairs.retain(|&(a, b), _| a != nation && b != nation);
        self.proposals
            .retain(|proposal| proposal.from != nation && proposal.to != nation);
        self.reputation.remove(&nation);
        self.events.retain(|(recipient, _)| *recipient != nation);
    }

    /// Events since the last call, in the order they happened.
    pub(crate) fn take_events(&mut self) -> Vec<(NationId, DiplomaticEvent)> {
        std::mem::take(&mut self.events)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: NationId = NationId(0);
    const B: NationId = NationId(1);
    const C: NationId = NationId(2);
    const NATIONS: [NationId; 3] = [A, B, C];

    fn apply(
        diplomacy: &mut Diplomacy,
        actor: NationId,
        action: DiplomaticAction,
    ) -> DiplomaticOutcome {
        diplomacy.apply(actor, action, &NATIONS).unwrap()
    }

    #[test]
    fn relations_start_neutral_and_are_symmetric() {
        let mut diplomacy = Diplomacy::default();
        assert_eq!(diplomacy.relation(A, B), Relation::Neutral);
        apply(
            &mut diplomacy,
            A,
            DiplomaticAction::DeclareEmbargo { target: B },
        );
        assert_eq!(
            diplomacy.relation(B, A),
            Relation::Embargo { imposed_by: A }
        );
        assert!(!diplomacy.relation(A, B).allows_trade());
        assert_eq!(diplomacy.relation(A, C), Relation::Neutral);
    }

    #[test]
    fn tariffs_follow_the_relation() {
        assert_eq!(Relation::Neutral.tariff_rate(), Some(0.2));
        assert_eq!(Relation::Truce { turns_left: 3 }.tariff_rate(), Some(0.2));
        assert_eq!(Relation::TradeAgreement.tariff_rate(), Some(0.05));
        assert_eq!(Relation::Alliance.tariff_rate(), Some(0.0));
        assert_eq!(Relation::War { aggressor: A }.tariff_rate(), None);
        assert_eq!(Relation::Embargo { imposed_by: A }.tariff_rate(), None);
    }

    #[test]
    fn a_proposal_is_answerable_only_in_the_next_turn() {
        let mut diplomacy = Diplomacy::default();
        let respond = DiplomaticAction::Respond {
            proposer: A,
            treaty: Treaty::TradeAgreement,
            accept: true,
        };
        apply(
            &mut diplomacy,
            A,
            DiplomaticAction::Propose {
                target: B,
                treaty: Treaty::TradeAgreement,
            },
        );
        assert!(matches!(
            diplomacy.check(B, respond),
            Err(DiplomacyError::NoSuchProposal { .. })
        ));
        diplomacy.advance_turn();
        assert_eq!(diplomacy.check(B, respond), Ok(()));
        diplomacy.advance_turn();
        assert!(diplomacy.proposals().is_empty(), "expired unanswered");
    }

    #[test]
    fn the_truce_ends_in_neutrality_after_ten_turns() {
        let mut diplomacy = Diplomacy::default();
        diplomacy.set_relation(A, B, Relation::Truce { turns_left: 10 });
        for left in (1..10).rev() {
            diplomacy.advance_turn();
            assert_eq!(
                diplomacy.relation(A, B),
                Relation::Truce { turns_left: left }
            );
        }
        diplomacy.take_events();
        diplomacy.advance_turn();
        assert_eq!(diplomacy.relation(A, B), Relation::Neutral);
        assert_eq!(
            diplomacy.take_events(),
            vec![
                (A, DiplomaticEvent::TruceEnded { with: B }),
                (B, DiplomaticEvent::TruceEnded { with: A })
            ]
        );
    }

    #[test]
    fn removing_a_nation_forgets_everything_about_it() {
        let mut diplomacy = Diplomacy::default();
        apply(
            &mut diplomacy,
            A,
            DiplomaticAction::DeclareWar { target: B },
        );
        apply(
            &mut diplomacy,
            C,
            DiplomaticAction::Propose {
                target: B,
                treaty: Treaty::Alliance,
            },
        );
        diplomacy.remove_nation(B);
        assert_eq!(diplomacy.relation(A, B), Relation::Neutral);
        assert!(diplomacy.proposals().is_empty());
        assert!(diplomacy.take_events().iter().all(|(to, _)| *to != B));
    }
}
