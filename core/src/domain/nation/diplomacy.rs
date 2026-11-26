//! Diplomacy between nations (gameplay specification §8): relation states,
//! unilateral actions, bilateral proposals answered one turn later, wars and
//! peace of whole alliances with a truce, the warning period and reputation.
//!
//! Every pair of nations has one relation, `Neutral` until something changes
//! it. Each operation checks its rules against the current state and changes
//! nothing when it fails; World adds the checks that need the rest of the game
//! (whether the nations exist). The numbers (truce length, betrayal window,
//! reputation, tariffs) are content, loaded from `diplomacy.toml`.
use crate::domain::ids::NationId;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::Path;
use std::sync::{Arc, OnceLock};

/// The relation between two nations (§8.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum Relation {
    /// The default: trade with the neutral tariff.
    Neutral,
    /// No trade, no war. Only the imposing nation can lift it.
    Embargo { imposed_by: NationId },
    /// Trade with a low tariff.
    TradeAgreement,
    /// No tariff and passage for armies; allies go to war and make peace
    /// together (§8.3).
    Alliance,
    /// Armies may enter and fight; no trade. `aggressor` is the nation on
    /// the attacking side of this pair.
    War { aggressor: NationId },
    /// After peace: no war for `turns_left` more turns; trade as in neutrality.
    Truce { turns_left: u32 },
}

impl Relation {
    /// Settlements of the two nations may trade (§8.1, §9).
    pub fn allows_trade(self) -> bool {
        !matches!(self, Self::War { .. } | Self::Embargo { .. })
    }
}

/// An agreement that needs the consent of the other nation (§8.2): it is
/// proposed and takes effect only when the other nation accepts it. War and
/// embargo are not treaties: they are unilateral and take effect at once.
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

/// The answer to the nation that took a diplomatic action, in the result of
/// its decision. What the action did to other nations reaches them as
/// [`DiplomaticEvent`]s in their turn reports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "effect", rename_all = "snake_case", deny_unknown_fields)]
pub enum DiplomaticOutcome {
    /// Wars are fought by whole alliances (§8.3): `attackers_joined` are
    /// allies of the aggressor and `defenders_joined` allies of the target
    /// that entered the war, ascending ID.
    WarDeclared {
        target: NationId,
        attackers_joined: Vec<NationId>,
        defenders_joined: Vec<NationId>,
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
        /// Peace is made by whole alliances: the other pairs of the two
        /// alliances whose war ended in a truce with it (lower ID first).
        allied_truces: Vec<[NationId; 2]>,
    },
    ProposalRejected {
        proposer: NationId,
        treaty: Treaty,
    },
}

/// Something that happened to a nation, for its turn report: the effects of
/// other nations' actions on it and changes that need no action (expired
/// proposals, ended truces).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case", deny_unknown_fields)]
pub enum DiplomaticEvent {
    /// `aggressor` declared war on this nation.
    WarDeclared {
        aggressor: NationId,
    },
    /// This nation is now at war with `enemy` because its alliance or the
    /// enemy's went to war when `aggressor` declared war on `target`.
    WarJoined {
        enemy: NationId,
        aggressor: NationId,
        target: NationId,
    },
    /// This nation's war with `enemy` became a truce because `proposer` and
    /// `accepter`, members of the two alliances, made peace.
    PeaceJoined {
        enemy: NationId,
        proposer: NationId,
        accepter: NationId,
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

/// Customs duty per relation (§9), as a fraction of the traded value.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TariffRates {
    /// Neutrality and truce.
    pub neutral: f32,
    pub trade_agreement: f32,
    pub alliance: f32,
}

/// The numbers of diplomacy, loaded from `diplomacy.toml`.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiplomacyRules {
    /// Length of the truce after peace.
    pub truce_turns: u32,
    /// A war declared within this many turns after breaking a treaty with the
    /// same nation counts against the reputation again.
    pub betrayal_window_turns: u32,
    /// Reputation added for breaking a trade agreement or an alliance.
    pub treaty_break_reputation: u32,
    /// Reputation added for a war within the betrayal window.
    pub betrayal_reputation: u32,
    pub tariffs: TariffRates,
}

#[derive(Debug)]
pub enum DiplomacyRulesError {
    Read(std::io::Error),
    Parse(toml::de::Error),
    /// The rules contradict themselves; the text names the rule.
    Invalid(&'static str),
}

impl fmt::Display for DiplomacyRulesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read(error) => write!(f, "cannot read diplomacy rules: {error}"),
            Self::Parse(error) => write!(f, "invalid diplomacy rules TOML: {error}"),
            Self::Invalid(rule) => write!(f, "invalid diplomacy rules: {rule}"),
        }
    }
}

impl std::error::Error for DiplomacyRulesError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read(error) => Some(error),
            Self::Parse(error) => Some(error),
            Self::Invalid(_) => None,
        }
    }
}

impl DiplomacyRules {
    /// The bundled `diplomacy.toml`.
    pub fn standard() -> &'static Self {
        static RULES: OnceLock<DiplomacyRules> = OnceLock::new();
        RULES.get_or_init(|| {
            Self::from_toml(include_str!("../../../../diplomacy.toml"))
                .expect("the bundled diplomacy rules must be valid")
        })
    }

    pub fn load(path: impl AsRef<Path>) -> Result<Self, DiplomacyRulesError> {
        let source = std::fs::read_to_string(path).map_err(DiplomacyRulesError::Read)?;
        Self::from_toml(&source)
    }

    pub fn from_toml(source: &str) -> Result<Self, DiplomacyRulesError> {
        let rules: Self = toml::from_str(source).map_err(DiplomacyRulesError::Parse)?;
        if rules.truce_turns == 0 {
            return Err(DiplomacyRulesError::Invalid("truce_turns must be positive"));
        }
        let tariffs = rules.tariffs;
        if ![tariffs.neutral, tariffs.trade_agreement, tariffs.alliance]
            .iter()
            .all(|rate| rate.is_finite() && (0.0..=1.0).contains(rate))
        {
            return Err(DiplomacyRulesError::Invalid(
                "tariffs must be fractions from 0.0 to 1.0",
            ));
        }
        Ok(rules)
    }

    /// Customs duty between two nations in this relation (§9), for the
    /// customs system; None when trade is blocked.
    pub fn tariff(&self, relation: Relation) -> Option<f32> {
        match relation {
            Relation::Neutral | Relation::Truce { .. } => Some(self.tariffs.neutral),
            Relation::TradeAgreement => Some(self.tariffs.trade_agreement),
            Relation::Alliance => Some(self.tariffs.alliance),
            Relation::War { .. } | Relation::Embargo { .. } => None,
        }
    }
}

/// Per-pair state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Pair {
    relation: Relation,
    /// The global turn in which the current relation started; 0 for a
    /// relation held since the start of the game.
    since_turn: u64,
    /// A treaty of this pair was broken this turn: no war until the next turn.
    warning: bool,
    /// The last treaty break of this pair: who broke it and how many turns ago.
    last_break: Option<(NationId, u32)>,
}

impl Default for Pair {
    fn default() -> Self {
        Self {
            relation: Relation::Neutral,
            since_turn: 0,
            warning: false,
            last_break: None,
        }
    }
}

/// All diplomatic state of the game.
#[derive(Debug)]
pub struct Diplomacy {
    rules: Arc<DiplomacyRules>,
    /// Keyed by the ordered pair (lower ID, higher ID); missing pairs are
    /// neutral since the start of the game.
    pairs: BTreeMap<(NationId, NationId), Pair>,
    proposals: Vec<Proposal>,
    /// Broken treaties per nation (§8.4); public.
    reputation: BTreeMap<NationId, u32>,
    /// Events not yet delivered to the nations' turn reports.
    events: Vec<(NationId, DiplomaticEvent)>,
}

impl Default for Diplomacy {
    fn default() -> Self {
        Self::new(Arc::new(*DiplomacyRules::standard()))
    }
}

fn key(a: NationId, b: NationId) -> (NationId, NationId) {
    (a.min(b), a.max(b))
}

/// Read access.
impl Diplomacy {
    pub fn new(rules: Arc<DiplomacyRules>) -> Self {
        Self {
            rules,
            pairs: BTreeMap::new(),
            proposals: Vec::new(),
            reputation: BTreeMap::new(),
            events: Vec::new(),
        }
    }

    pub fn rules(&self) -> &DiplomacyRules {
        &self.rules
    }

    /// The relation of two different nations.
    pub fn relation(&self, a: NationId, b: NationId) -> Relation {
        self.pairs
            .get(&key(a, b))
            .map_or(Relation::Neutral, |pair| pair.relation)
    }

    /// The global turn in which the current relation of the two started; 0
    /// when it has held since the start of the game.
    pub fn relation_since(&self, a: NationId, b: NationId) -> u64 {
        self.pairs.get(&key(a, b)).map_or(0, |pair| pair.since_turn)
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

    /// No relation other than neutrality and no proposal: replacing the rules
    /// cannot change anything in progress.
    pub(crate) fn is_untouched(&self) -> bool {
        self.proposals.is_empty()
            && self
                .pairs
                .values()
                .all(|pair| pair.relation == Relation::Neutral && pair.last_break.is_none())
    }

    pub(crate) fn set_rules(&mut self, rules: Arc<DiplomacyRules>) {
        self.rules = rules;
    }
}

/// Actions. Each one checks its rules first and changes nothing when it
/// fails. `turn` is the global turn being played; `nations` lists every nation
/// in the game, for the alliances.
impl Diplomacy {
    fn pair_mut(&mut self, a: NationId, b: NationId) -> &mut Pair {
        self.pairs.entry(key(a, b)).or_default()
    }

    fn set_relation(&mut self, a: NationId, b: NationId, relation: Relation, turn: u64) {
        let pair = self.pair_mut(a, b);
        pair.relation = relation;
        pair.since_turn = turn;
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

    fn not_self(actor: NationId, other: NationId) -> Result<(), DiplomacyError> {
        if actor == other {
            return Err(DiplomacyError::DiplomacyWithSelf);
        }
        Ok(())
    }

    /// Only from neutrality or an embargo, not during a truce and not in the
    /// warning turn. Both alliances go to war (§8.3).
    pub(crate) fn declare_war(
        &mut self,
        actor: NationId,
        target: NationId,
        nations: &[NationId],
        turn: u64,
    ) -> Result<DiplomaticOutcome, DiplomacyError> {
        Self::not_self(actor, target)?;
        match self.relation(actor, target) {
            Relation::Truce { turns_left } => {
                return Err(DiplomacyError::TruceInForce {
                    nation_id: target,
                    turns_left,
                });
            }
            Relation::Neutral | Relation::Embargo { .. } => {
                if self.in_warning_period(actor, target) {
                    return Err(DiplomacyError::WarningPeriod { nation_id: target });
                }
            }
            _ => return Err(self.invalid(actor, target)),
        }
        // §8.4: a war soon after breaking a treaty with the same nation
        // counts as another broken treaty.
        let last_break = self
            .pairs
            .get(&key(actor, target))
            .and_then(|pair| pair.last_break);
        if let Some((breaker, turns_ago)) = last_break
            && breaker == actor
            && turns_ago <= self.rules.betrayal_window_turns
        {
            *self.reputation.entry(actor).or_default() += self.rules.betrayal_reputation;
        }
        self.set_relation(actor, target, Relation::War { aggressor: actor }, turn);
        self.events
            .push((target, DiplomaticEvent::WarDeclared { aggressor: actor }));
        let (attackers_joined, defenders_joined) =
            self.join_alliance_war(actor, target, nations, turn);
        Ok(DiplomaticOutcome::WarDeclared {
            target,
            attackers_joined,
            defenders_joined,
        })
    }

    /// Only from neutrality.
    pub(crate) fn declare_embargo(
        &mut self,
        actor: NationId,
        target: NationId,
        turn: u64,
    ) -> Result<DiplomaticOutcome, DiplomacyError> {
        Self::not_self(actor, target)?;
        if self.relation(actor, target) != Relation::Neutral {
            return Err(self.invalid(actor, target));
        }
        self.set_relation(actor, target, Relation::Embargo { imposed_by: actor }, turn);
        self.events
            .push((target, DiplomaticEvent::EmbargoDeclared { by: actor }));
        Ok(DiplomaticOutcome::EmbargoDeclared { target })
    }

    /// Only the imposing nation; back to neutrality.
    pub(crate) fn lift_embargo(
        &mut self,
        actor: NationId,
        target: NationId,
        turn: u64,
    ) -> Result<DiplomaticOutcome, DiplomacyError> {
        Self::not_self(actor, target)?;
        match self.relation(actor, target) {
            Relation::Embargo { imposed_by } if imposed_by == actor => {}
            Relation::Embargo { .. } => {
                return Err(DiplomacyError::NotEmbargoImposer { nation_id: target });
            }
            _ => return Err(self.invalid(actor, target)),
        }
        self.set_relation(actor, target, Relation::Neutral, turn);
        self.events
            .push((target, DiplomaticEvent::EmbargoLifted { by: actor }));
        Ok(DiplomaticOutcome::EmbargoLifted { target })
    }

    /// Breaks a trade agreement or an alliance: back to neutrality, a warning
    /// turn starts and the reputation suffers. A truce is not a treaty that
    /// can be broken.
    pub(crate) fn break_treaty(
        &mut self,
        actor: NationId,
        target: NationId,
        treaty: Treaty,
        turn: u64,
    ) -> Result<DiplomaticOutcome, DiplomacyError> {
        Self::not_self(actor, target)?;
        let in_force = match treaty {
            Treaty::TradeAgreement => Relation::TradeAgreement,
            Treaty::Alliance => Relation::Alliance,
            Treaty::Peace => return Err(self.invalid(actor, target)),
        };
        if self.relation(actor, target) != in_force {
            return Err(self.invalid(actor, target));
        }
        self.set_relation(actor, target, Relation::Neutral, turn);
        let pair = self.pair_mut(actor, target);
        pair.warning = true;
        pair.last_break = Some((actor, 0));
        *self.reputation.entry(actor).or_default() += self.rules.treaty_break_reputation;
        self.events
            .push((target, DiplomaticEvent::TreatyBroken { by: actor, treaty }));
        Ok(DiplomaticOutcome::TreatyBroken { target, treaty })
    }

    /// The target can answer in the next turn; at most one pending proposal
    /// of each kind to each nation.
    pub(crate) fn propose(
        &mut self,
        actor: NationId,
        target: NationId,
        treaty: Treaty,
    ) -> Result<DiplomaticOutcome, DiplomacyError> {
        Self::not_self(actor, target)?;
        if !treaty.can_start_from(self.relation(actor, target)) {
            return Err(self.invalid(actor, target));
        }
        if self.proposal_index(actor, target, treaty).is_some() {
            return Err(DiplomacyError::ProposalAlreadyPending {
                nation_id: target,
                treaty,
            });
        }
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
        Ok(DiplomaticOutcome::ProposalSent { target, treaty })
    }

    /// Answers a proposal made in the previous turn. Acceptance checks the
    /// transition again: the relation may have changed since the proposal.
    /// A peace is made by whole alliances and keeps every settlement with its
    /// current owner (§8.5).
    pub(crate) fn respond(
        &mut self,
        actor: NationId,
        proposer: NationId,
        treaty: Treaty,
        accept: bool,
        nations: &[NationId],
        turn: u64,
    ) -> Result<DiplomaticOutcome, DiplomacyError> {
        Self::not_self(actor, proposer)?;
        let index = self
            .proposal_index(proposer, actor, treaty)
            .filter(|&index| self.proposals[index].answerable)
            .ok_or(DiplomacyError::NoSuchProposal {
                nation_id: proposer,
                treaty,
            })?;
        if accept && !treaty.can_start_from(self.relation(actor, proposer)) {
            return Err(self.invalid(actor, proposer));
        }
        self.proposals.remove(index);
        if !accept {
            self.events.push((
                proposer,
                DiplomaticEvent::ProposalRejected { by: actor, treaty },
            ));
            return Ok(DiplomaticOutcome::ProposalRejected { proposer, treaty });
        }
        let relation = match treaty {
            Treaty::TradeAgreement => Relation::TradeAgreement,
            Treaty::Alliance => Relation::Alliance,
            Treaty::Peace => Relation::Truce {
                turns_left: self.rules.truce_turns,
            },
        };
        self.set_relation(actor, proposer, relation, turn);
        self.events.push((
            proposer,
            DiplomaticEvent::ProposalAccepted { by: actor, treaty },
        ));
        let allied_truces = if treaty == Treaty::Peace {
            self.join_alliance_peace(proposer, actor, nations, turn)
        } else {
            Vec::new()
        };
        Ok(DiplomaticOutcome::ProposalAccepted {
            proposer,
            treaty,
            relation,
            allied_truces,
        })
    }

    /// `leader` and its allies in a conflict with `opponent`: every nation
    /// allied with the leader, except the opponent and nations allied with
    /// the opponent as well (they stay out). Allies of allies are not members.
    fn alliance(
        &self,
        leader: NationId,
        opponent: NationId,
        nations: &[NationId],
    ) -> Vec<NationId> {
        let mut members = vec![leader];
        members.extend(nations.iter().copied().filter(|&nation| {
            nation != leader
                && nation != opponent
                && self.relation(leader, nation) == Relation::Alliance
                && self.relation(opponent, nation) != Relation::Alliance
        }));
        members
    }

    /// §8.3, wars of whole alliances: after `aggressor` declared war on
    /// `target`, every member of the aggressor's alliance is at war with every
    /// member of the target's. This overrides neutrality, embargoes, trade
    /// agreements and truces between them; pairs already at war keep their
    /// aggressor, and two members that are allied with each other do not
    /// fight. The member of the aggressor's side is the aggressor of each new
    /// pair. Returns the allies of each side that entered a new war.
    fn join_alliance_war(
        &mut self,
        aggressor: NationId,
        target: NationId,
        nations: &[NationId],
        turn: u64,
    ) -> (Vec<NationId>, Vec<NationId>) {
        let attackers = self.alliance(aggressor, target, nations);
        let defenders = self.alliance(target, aggressor, nations);
        let mut attackers_joined = BTreeSet::new();
        let mut defenders_joined = BTreeSet::new();
        for &attacker in &attackers {
            for &defender in &defenders {
                if (attacker, defender) == (aggressor, target) {
                    continue;
                }
                match self.relation(attacker, defender) {
                    Relation::Alliance | Relation::War { .. } => continue,
                    _ => {}
                }
                self.set_relation(
                    attacker,
                    defender,
                    Relation::War {
                        aggressor: attacker,
                    },
                    turn,
                );
                for (nation, enemy) in [(attacker, defender), (defender, attacker)] {
                    self.events.push((
                        nation,
                        DiplomaticEvent::WarJoined {
                            enemy,
                            aggressor,
                            target,
                        },
                    ));
                }
                if attacker != aggressor {
                    attackers_joined.insert(attacker);
                }
                if defender != target {
                    defenders_joined.insert(defender);
                }
            }
        }
        (
            attackers_joined.into_iter().collect(),
            defenders_joined.into_iter().collect(),
        )
    }

    /// §8.5, peace of whole alliances: after `proposer` and `accepter` made
    /// peace, every war between a member of one's alliance and a member of
    /// the other's becomes a truce as well. Returns those other pairs.
    fn join_alliance_peace(
        &mut self,
        proposer: NationId,
        accepter: NationId,
        nations: &[NationId],
        turn: u64,
    ) -> Vec<[NationId; 2]> {
        let first = self.alliance(proposer, accepter, nations);
        let second = self.alliance(accepter, proposer, nations);
        let truce = Relation::Truce {
            turns_left: self.rules.truce_turns,
        };
        let mut truces = Vec::new();
        for &a in &first {
            for &b in &second {
                if key(a, b) == key(proposer, accepter)
                    || !matches!(self.relation(a, b), Relation::War { .. })
                {
                    continue;
                }
                self.set_relation(a, b, truce, turn);
                for (nation, enemy) in [(a, b), (b, a)] {
                    self.events.push((
                        nation,
                        DiplomaticEvent::PeaceJoined {
                            enemy,
                            proposer,
                            accepter,
                        },
                    ));
                }
                let (low, high) = key(a, b);
                truces.push([low, high]);
            }
        }
        truces.sort();
        truces
    }
}

/// Turn bookkeeping.
impl Diplomacy {
    /// End of global turn `turn`: truces run down (ending in neutrality from
    /// the next turn), warning periods end, break ages grow, unanswered
    /// proposals of the answer turn expire and this turn's proposals become
    /// answerable.
    pub(crate) fn advance_turn(&mut self, turn: u64) {
        let window = self.rules.betrayal_window_turns;
        let mut ended = Vec::new();
        for (&(a, b), pair) in &mut self.pairs {
            pair.warning = false;
            if let Some((_, turns_ago)) = &mut pair.last_break {
                *turns_ago += 1;
            }
            if pair
                .last_break
                .is_some_and(|(_, turns_ago)| turns_ago > window)
            {
                pair.last_break = None;
            }
            if let Relation::Truce { turns_left } = &mut pair.relation {
                *turns_left = turns_left.saturating_sub(1);
                if *turns_left == 0 {
                    pair.relation = Relation::Neutral;
                    pair.since_turn = turn + 1;
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

    #[test]
    fn relations_start_neutral_and_are_symmetric() {
        let mut diplomacy = Diplomacy::default();
        assert_eq!(diplomacy.relation(A, B), Relation::Neutral);
        assert_eq!(diplomacy.relation_since(A, B), 0);
        diplomacy.declare_embargo(A, B, 3).unwrap();
        assert_eq!(
            diplomacy.relation(B, A),
            Relation::Embargo { imposed_by: A }
        );
        assert_eq!(diplomacy.relation_since(B, A), 3);
        assert!(!diplomacy.relation(A, B).allows_trade());
        assert_eq!(diplomacy.relation(A, C), Relation::Neutral);
        assert_eq!(
            diplomacy.declare_embargo(A, A, 3),
            Err(DiplomacyError::DiplomacyWithSelf)
        );
    }

    #[test]
    fn the_bundled_rules_follow_the_specification() {
        let rules = DiplomacyRules::standard();
        assert_eq!(rules.truce_turns, 10);
        assert_eq!(rules.betrayal_window_turns, 5);
        assert_eq!(
            (rules.treaty_break_reputation, rules.betrayal_reputation),
            (1, 1)
        );
        assert_eq!(rules.tariff(Relation::Neutral), Some(0.2));
        assert_eq!(rules.tariff(Relation::Truce { turns_left: 3 }), Some(0.2));
        assert_eq!(rules.tariff(Relation::TradeAgreement), Some(0.05));
        assert_eq!(rules.tariff(Relation::Alliance), Some(0.0));
        assert_eq!(rules.tariff(Relation::War { aggressor: A }), None);
        assert_eq!(rules.tariff(Relation::Embargo { imposed_by: A }), None);
    }

    #[test]
    fn invalid_rules_are_rejected_when_loading() {
        let source = include_str!("../../../../diplomacy.toml");
        assert!(DiplomacyRules::from_toml(source).is_ok());
        for (from, to) in [
            ("truce_turns = 10", "truce_turns = 0"),
            ("neutral = 0.2", "neutral = 1.5"),
            ("alliance = 0.0", "alliance = -0.1"),
        ] {
            assert!(
                matches!(
                    DiplomacyRules::from_toml(&source.replace(from, to)),
                    Err(DiplomacyRulesError::Invalid(_))
                ),
                "{to}"
            );
        }
        assert!(matches!(
            DiplomacyRules::from_toml(&format!("{source}\nwar_weariness = 3\n")),
            Err(DiplomacyRulesError::Parse(_))
        ));
    }

    #[test]
    fn a_proposal_is_answerable_only_in_the_next_turn() {
        let mut diplomacy = Diplomacy::default();
        diplomacy.propose(A, B, Treaty::TradeAgreement).unwrap();
        assert!(matches!(
            diplomacy.respond(B, A, Treaty::TradeAgreement, true, &NATIONS, 1),
            Err(DiplomacyError::NoSuchProposal { .. })
        ));
        diplomacy.advance_turn(1);
        assert!(
            diplomacy
                .respond(B, A, Treaty::TradeAgreement, true, &NATIONS, 2)
                .is_ok()
        );
        assert_eq!(diplomacy.relation(A, B), Relation::TradeAgreement);
        assert_eq!(diplomacy.relation_since(A, B), 2);
        diplomacy.propose(A, C, Treaty::Alliance).unwrap();
        diplomacy.advance_turn(2);
        diplomacy.advance_turn(3);
        assert!(diplomacy.proposals().is_empty(), "expired unanswered");
    }

    #[test]
    fn the_truce_ends_in_neutrality_after_the_configured_turns() {
        let rules = DiplomacyRules {
            truce_turns: 3,
            ..*DiplomacyRules::standard()
        };
        let mut diplomacy = Diplomacy::new(Arc::new(rules));
        diplomacy.declare_war(A, B, &NATIONS, 1).unwrap();
        diplomacy.propose(A, B, Treaty::Peace).unwrap();
        diplomacy.advance_turn(1);
        diplomacy
            .respond(B, A, Treaty::Peace, true, &NATIONS, 2)
            .unwrap();
        assert_eq!(diplomacy.relation(A, B), Relation::Truce { turns_left: 3 });
        diplomacy.advance_turn(2);
        diplomacy.advance_turn(3);
        diplomacy.take_events();
        diplomacy.advance_turn(4);
        assert_eq!(diplomacy.relation(A, B), Relation::Neutral);
        assert_eq!(
            diplomacy.relation_since(A, B),
            5,
            "neutral from the next turn"
        );
        assert_eq!(
            diplomacy.take_events(),
            vec![
                (A, DiplomaticEvent::TruceEnded { with: B }),
                (B, DiplomaticEvent::TruceEnded { with: A })
            ]
        );
    }

    #[test]
    fn a_truce_is_not_a_treaty_that_can_be_broken() {
        let mut diplomacy = Diplomacy::default();
        assert!(matches!(
            diplomacy.break_treaty(A, B, Treaty::Peace, 1),
            Err(DiplomacyError::InvalidRelation { .. })
        ));
        assert!(matches!(
            diplomacy.break_treaty(A, B, Treaty::Alliance, 1),
            Err(DiplomacyError::InvalidRelation { .. })
        ));
        assert_eq!(
            diplomacy.reputation(A),
            0,
            "a failed action changes nothing"
        );
    }

    #[test]
    fn removing_a_nation_forgets_everything_about_it() {
        let mut diplomacy = Diplomacy::default();
        diplomacy.declare_war(A, B, &NATIONS, 1).unwrap();
        diplomacy.propose(C, B, Treaty::Alliance).unwrap();
        diplomacy.remove_nation(B);
        assert_eq!(diplomacy.relation(A, B), Relation::Neutral);
        assert!(diplomacy.proposals().is_empty());
        assert!(diplomacy.take_events().iter().all(|(to, _)| *to != B));
    }
}
