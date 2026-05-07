pub(super) struct ItemSlot {
    pub output_index: i64,
    pub canonical_index: u32,
    pub kind: SlotKind,
}

pub(super) enum SlotKind {
    Message,
    Function,
    Reasoning,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum SlotKindMatch {
    Message,
    Function,
    Reasoning,
}

pub(super) fn lookup_canonical(
    items: &[ItemSlot],
    output_index: i64,
    want: SlotKindMatch,
) -> Option<u32> {
    items.iter().find_map(|s| {
        let kind_match = matches!(
            (&s.kind, want),
            (SlotKind::Message, SlotKindMatch::Message)
                | (SlotKind::Function, SlotKindMatch::Function)
                | (SlotKind::Reasoning, SlotKindMatch::Reasoning)
        );
        (s.output_index == output_index && kind_match).then_some(s.canonical_index)
    })
}
