"""Preserve source component-class rules and reference visibility in a panel.

KiKit 1.8 namespaces nets, but not Reference comparisons or KiCad 10 component
class assignments. Its bakeRef option also makes hidden references visible.
Keep these compatibility fixes separate from manufacturing-rule policy.
"""

from __future__ import annotations

from copy import deepcopy
import re


REFERENCE_COMPARISON = re.compile(
    r"([AB]\s*\.\s*Reference\s*[=!]=\s*)'([^']*)'", re.IGNORECASE
)


def namespace_reference_conditions(
    condition: str, prefix: str, references: set[str]
) -> str:
    def replace(match: re.Match) -> str:
        reference = match[2]
        if reference not in references:
            raise ValueError(f"Cannot namespace unknown rule reference: {reference!r}")
        return f"{match[1]}'{prefix}-{reference}'"

    return REFERENCE_COMPARISON.sub(replace, condition)


def namespace_component_classes(
    project: dict, prefix: str, references: set[str]
) -> list[dict]:
    """Copy explicit reference-list assignments; fail closed on unsupported filters.

    Our source projects use only this form. Wildcard, sheet, value, or combined
    filters need an explicit evaluator before they can safely cross namespaces.
    """
    settings = project.get("component_class_settings", {})
    if settings.get("sheet_component_classes", {}).get("enabled", False):
        raise ValueError(
            "Panel component-class inheritance does not support sheet rules"
        )
    result = []
    for original in settings.get("assignments", []):
        assignment = deepcopy(original)
        conditions = assignment.get("conditions", {})
        reference_filter = conditions.get("REFERENCE", {})
        if (
            set(conditions) != {"REFERENCE"}
            or set(reference_filter) != {"primary"}
            or assignment.get("conditions_operator") not in {"ALL", "ANY"}
        ):
            raise ValueError(f"Unsupported panel component-class filter: {original}")
        refs = [r.strip() for r in reference_filter["primary"].split(",")]
        if not refs or any(r not in references for r in refs):
            raise ValueError(
                f"Unknown or nonliteral component-class references: {refs}"
            )
        reference_filter["primary"] = ",".join(f"{prefix}-{ref}" for ref in refs)
        result.append(assignment)
    return result


def bake_visible_reference(board, reference, original_text: str, make_text) -> bool:
    """Bake only visible references, on their original layer (including Fab).

    The reference remains the unique panel identifier used for assembly data;
    the graphic retains the individual board's readable designator.
    """
    if not reference.IsVisible():
        return False
    text = make_text(board)
    text.SetText(original_text)
    text.SetTextX(reference.GetTextPos()[0])
    text.SetTextY(reference.GetTextPos()[1])
    text.SetTextThickness(reference.GetTextThickness())
    text.SetTextSize(reference.GetTextSize())
    text.SetHorizJustify(reference.GetHorizJustify())
    text.SetVertJustify(reference.GetVertJustify())
    text.SetTextAngle(reference.GetTextAngle())
    text.SetLayer(reference.GetLayer())
    text.SetMirrored(reference.IsMirrored())
    text.SetItalic(reference.IsItalic())
    text.SetBold(reference.IsBold())
    board.Add(text)
    reference.SetVisible(False)
    return True
