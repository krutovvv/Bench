#!/usr/bin/env python3
"""Write the syntax themes of assets/themes/*.json from role colours.

Every theme here carries every role the kit's `SyntaxColors` knows (the test in
`crates/ui/src/theme/palettes.rs` checks it) and the status colours, so nothing is left to the
kit's default by accident. The key names are the kit's own: `comment_doc`, not `comment.doc`.
"""
import json, pathlib

OUT = pathlib.Path(__file__).resolve().parent.parent / "assets" / "themes"

# role -> the key it fills in the JSON
def theme(name, appearance, bg, fg, line, lineno, lineno_on, invisible, r):
    s = {
        "attribute": {"color": "#" + r["attr"]},
        "boolean": {"color": "#" + r["num"]},
        "comment": {"color": "#" + r["comment"], "font_style": "italic"},
        # The kit's own key name: `comment.doc` parses as an unknown key and is dropped.
        "comment_doc": {"color": "#" + r["comment_doc"], "font_style": "italic"},
        "constant": {"color": "#" + r["constant"]},
        "constructor": {"color": "#" + r["fn"]},
        "embedded": {"color": "#" + fg},
        "emphasis": {"font_style": "italic"},
        "emphasis.strong": {"font_weight": 700},
        "enum": {"color": "#" + r["enum"]},
        "function": {"color": "#" + r["fn"]},
        "hint": {"color": "#" + r["hint"]},
        "keyword": {"color": "#" + r["kw"]},
        "label": {"color": "#" + r["attr"]},
        "link_text": {"color": "#" + r["fn"], "font_style": "normal"},
        "link_uri": {"color": "#" + r["cyan"], "font_style": "italic"},
        "number": {"color": "#" + r["num"]},
        "operator": {"color": "#" + r["op"]},
        "property": {"color": "#" + r["prop"]},
        "punctuation": {"color": "#" + r["punct"]},
        "punctuation.bracket": {"color": "#" + r["punct2"]},
        "punctuation.delimiter": {"color": "#" + r["punct2"]},
        "punctuation.list_marker": {"color": "#" + r["red"]},
        "punctuation.special": {"color": "#" + r["escape"]},
        "string": {"color": "#" + r["string"]},
        "string.escape": {"color": "#" + r["escape"]},
        "string.regex": {"color": "#" + r["regex"]},
        "string.special": {"color": "#" + r["num"]},
        "string.special.symbol": {"color": "#" + r["num"]},
        "tag": {"color": "#" + r["tag"]},
        "tag.doctype": {"color": "#" + r["kw"]},
        "text.literal": {"color": "#" + r["string"]},
        "text.code.span": {"color": "#" + r["string"]},
        "title": {"color": "#" + r["red"], "font_weight": 700},
        "type": {"color": "#" + r["type"]},
        "variable": {"color": "#" + r["var"]},
        "variable.special": {"color": "#" + r["num"]},
        "primary": {"color": "#" + fg},
        "predictive": {"color": "#" + r["comment"]},
        "preproc": {"color": "#" + r["preproc"]},
        "variant": {"color": "#" + r["variant"]},
    }
    return {
        "name": name,
        "appearance": appearance,
        "style": {
            "editor.background": "#" + bg,
            "editor.foreground": "#" + fg,
            "editor.active_line.background": "#" + line,
            "editor.line_number": "#" + lineno,
            "editor.active_line_number": "#" + lineno_on,
            "editor.invisible": "#" + invisible,
            # The status colours the kit reads from here (it blends the backgrounds and borders
            # itself): the editor's diagnostics, the Problems tab, a field's error.
            "error": "#" + r["err"],
            "warning": "#" + r["warn"],
            "success": "#" + r["ok"],
            "info": "#" + r["info"],
            "hint": "#" + r["hint"],
            "syntax": s,
        },
    }


THEMES = {
    # Bench's two halves: GitHub's role colours on Bench's own surfaces (palettes.rs).
    "bench-dark": theme(
        "Bench Dark", "dark", "1A1B1F", "E4E5E8", "25272C", "6E717A", "E4E5E8", "3A3C43",
        dict(kw="FF7B72", string="A5D6FF", num="79C0FF", constant="79C0FF", fn="D2A8FF",
             type="FFA657", var="FFA657", prop="79C0FF", tag="7EE787", attr="79C0FF",
             op="FF7B72", comment="8B949E", comment_doc="8B949E", escape="A5D6FF",
             regex="A5D6FF", enum="FFA657", punct="C9D1D9", punct2="8B949E",
             red="F85149", cyan="79C0FF",
             hint="8B949E", preproc="FF7B72", variant="79C0FF", err="F85149", warn="D29922", ok="3FB950", info="58A6FF"),
    ),
    "bench-light": theme(
        "Bench Light", "light", "FFFFFF", "1B1C1F", "F6F6F7", "989BA3", "1B1C1F", "DCDDE2",
        dict(kw="CF222E", string="0A3069", num="0550AE", constant="0550AE", fn="8250DF",
             type="953800", var="953800", prop="0550AE", tag="116329", attr="0550AE",
             op="CF222E", comment="6E7781", comment_doc="6E7781", escape="0A3069",
             regex="0A3069", enum="953800", punct="1F2328", punct2="656D76",
             red="CF222E", cyan="0550AE",
             hint="6E7781", preproc="CF222E", variant="0550AE", err="CF222E", warn="BF8700", ok="1A7F37", info="0969DA"),
    ),
}

for name, doc in THEMES.items():
    path = OUT / f"{name}.json"
    path.write_text(json.dumps(doc, indent=2) + "\n")
    print(path.name, len(doc["style"]["syntax"]))