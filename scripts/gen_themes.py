#!/usr/bin/env python3
"""Write the syntax themes of assets/themes/*.json from role colours.

Every theme here is written to the same key set as assets/themes/one-dark.json, so
no scope is left to the kit's default by accident.
"""
import json, pathlib

OUT = pathlib.Path(__file__).resolve().parent.parent / "assets" / "themes"

# role -> the key it fills in the JSON
def theme(name, appearance, bg, fg, line, lineno, lineno_on, invisible, r):
    s = {
        "attribute": {"color": "#" + r["attr"]},
        "boolean": {"color": "#" + r["num"]},
        "comment": {"color": "#" + r["comment"], "font_style": "italic"},
        "comment.doc": {"color": "#" + r["comment_doc"], "font_style": "italic"},
        "constant": {"color": "#" + r["constant"]},
        "constructor": {"color": "#" + r["fn"]},
        "embedded": {"color": "#" + fg},
        "emphasis": {"font_style": "italic"},
        "emphasis.strong": {"font_weight": 700},
        "enum": {"color": "#" + r["enum"]},
        "function": {"color": "#" + r["fn"]},
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
            "syntax": s,
        },
    }


THEMES = {
    # One Dark Pro (dark) and One Light Pro (light), the VS Code family.
    "one-dark-pro": theme(
        "One Dark Pro", "dark", "282C34", "ABB2BF", "2C313C", "4B5263", "ABB2BF", "5C6370",
        dict(kw="C678DD", string="98C379", num="D19A66", constant="56B6C2", fn="61AFEF",
             type="E5C07B", var="E06C75", prop="E06C75", tag="E06C75", attr="D19A66",
             op="56B6C2", comment="5C6370", comment_doc="7F848E", escape="56B6C2",
             regex="56B6C2", enum="E5C07B", punct="ABB2BF", punct2="B2B9C6",
             red="E06C75", cyan="56B6C2"),
    ),
    "one-light-pro": theme(
        "One Light Pro", "light", "FAFAFA", "383A42", "EFEFEF", "A0A1A7", "383A42", "CACAC9",
        dict(kw="A626A4", string="50A14F", num="986801", constant="986801", fn="4078F2",
             type="C18401", var="E45649", prop="E45649", tag="E45649", attr="986801",
             op="A626A4", comment="A0A1A7", comment_doc="4078F2", escape="4078F2",
             regex="3E998F", enum="C18401", punct="383A42", punct2="383A42",
             red="E45649", cyan="0184BC"),
    ),
    # Dracula: the dark half of the family; its light half is Bench's.
    "dracula": theme(
        "Dracula", "dark", "282A36", "F8F8F2", "44475A", "6272A2", "F8F8F2", "6272A2",
        dict(kw="FF79C6", string="F1FA8C", num="BD93F9", constant="BD93F9", fn="50FA7B",
             type="8BE9FD", var="F8F8F2", prop="8BE9FD", tag="50FA7B", attr="50FA7B",
             op="FF79C6", comment="6272A2", comment_doc="6272A2", escape="BD93F9",
             regex="F1FA8C", enum="8BE9FD", punct="F8F8F2", punct2="F8F8F2",
             red="FF5555", cyan="8BE9FD"),
    ),
    # Visual Studio Code's own defaults.
    "vs-dark": theme(
        "VS Code Dark+", "dark", "1E1E1E", "D4D4D4", "2A2D2E", "858585", "C6C6C6", "737373",
        dict(kw="569CD6", string="CE9178", num="B5CEA8", constant="4FC1FF", fn="DCDCAA",
             type="4EC9B0", var="9CDCFE", prop="9CDCFE", tag="569CD6", attr="9CDCFE",
             op="D4D4D4", comment="6A9955", comment_doc="6A9955", escape="569CD6",
             regex="D16969", enum="4EC9B0", punct="D4D4D4", punct2="D4D4D4",
             red="F44747", cyan="4FC1FF"),
    ),
    "vs-light": theme(
        "VS Code Light+", "light", "FFFFFF", "000000", "F2F2F2", "237893", "0B216F", "A5A5A5",
        dict(kw="0000FF", string="A31515", num="098658", constant="0070C1", fn="795E26",
             type="267F99", var="001080", prop="001080", tag="800000", attr="FF0000",
             op="000000", comment="008000", comment_doc="008000", escape="0000FF",
             regex="811F3F", enum="267F99", punct="000000", punct2="000000",
             red="CD3131", cyan="0451A5"),
    ),
    # JetBrains: Darcula and IntelliJ Light.
    "darcula": theme(
        "Darcula", "dark", "2B2B2B", "A9B7C6", "323436", "606366", "A9B7C6", "505050",
        dict(kw="CC7832", string="6A8759", num="6897BB", constant="9876AA", fn="FFC66D",
             type="A9B7C6", var="A9B7C6", prop="9876AA", tag="E8BF6A", attr="BABABA",
             op="CC7832", comment="808080", comment_doc="629755", escape="CC7832",
             regex="6A8759", enum="A9B7C6", punct="A9B7C6", punct2="CC7832",
             red="FF6B68", cyan="6897BB"),
    ),
    "intellij-light": theme(
        "IntelliJ Light", "light", "FFFFFF", "000000", "F2F2F2", "737373", "000000", "A0A0A0",
        dict(kw="0033B3", string="067D17", num="1750EB", constant="0000C0", fn="00627A",
             type="A02AA6", var="000000", prop="871094", tag="000080", attr="871094",
             op="000000", comment="8C8C8C", comment_doc="526D8A", escape="067D17",
             regex="871094", enum="A02AA6", punct="000000", punct2="000000",
             red="D80606", cyan="0000C0"),
    ),
    # GitHub's own themes.
    "github-dark": theme(
        "GitHub Dark", "dark", "0D1117", "C9D1D9", "161B22", "6E7681", "C9D1D9", "484F58",
        dict(kw="FF7B72", string="A5D6FF", num="79C0FF", constant="79C0FF", fn="D2A8FF",
             type="FFA657", var="FFA657", prop="79C0FF", tag="7EE787", attr="79C0FF",
             op="FF7B72", comment="8B949E", comment_doc="8B949E", escape="A5D6FF",
             regex="A5D6FF", enum="FFA657", punct="C9D1D9", punct2="8B949E",
             red="F85149", cyan="79C0FF"),
    ),
    "github-light": theme(
        "GitHub Light", "light", "FFFFFF", "1F2328", "F6F8FA", "8C959F", "1F2328", "D0D7DE",
        dict(kw="CF222E", string="0A3069", num="0550AE", constant="0550AE", fn="8250DF",
             type="953800", var="953800", prop="0550AE", tag="116329", attr="0550AE",
             op="CF222E", comment="6E7781", comment_doc="6E7781", escape="0A3069",
             regex="0A3069", enum="953800", punct="1F2328", punct2="656D76",
             red="CF222E", cyan="0550AE"),
    ),
}

for name, doc in THEMES.items():
    path = OUT / f"{name}.json"
    path.write_text(json.dumps(doc, indent=2) + "\n")
    print(path.name, len(doc["style"]["syntax"]))