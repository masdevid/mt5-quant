#!/usr/bin/env python3
"""List incomplete OpenSSF Best Practices criteria and generate a pre-filled edit URL.

Usage:
  cii_criteria.py <project_id> [section] [--exclude n1,n2,n3]

  section: passing | silver | gold | baseline-1 | baseline-2 | baseline-3
           (default: passing)
  --exclude: comma-separated criterion names to SKIP from the pre-filled URL
             (they stay at their current Unmet/?/N/A in the form).

The script fetches the upstream criteria.yml to learn which criteria belong to
which tier/section, then restricts BOTH the printed incomplete list AND the
generated edit-URL parameters to only that section's criteria.
"""
import sys, os, json, urllib.request, urllib.parse, argparse
import ssl

try:
    import certifi
    _SSL_CTX = ssl.create_default_context(cafile=certifi.where())
except Exception:
    _SSL_CTX = ssl.create_default_context()

BASE = "https://www.bestpractices.dev"
CRITERIA_URL = ("https://raw.githubusercontent.com/ossf/best-practices-badge/"
                "main/criteria/criteria.yml")

# CLI section argument -> tier key inside criteria.yml
SECTION_TO_TIER = {
    "passing": "0",
    "silver": "1",
    "gold": "2",
    "baseline-1": "baseline-1",
    "baseline-2": "baseline-2",
    "baseline-3": "baseline-3",
}


def load_justifications():
    """Load real justifications from the sibling cii_justifications.json.

    Returns a dict name -> justification string. Falls back to {} if the file
    is missing or invalid, so callers should still default to a placeholder.
    """
    path = os.path.join(os.path.dirname(os.path.abspath(__file__)),
                        "cii_justifications.json")
    try:
        with open(path, "r", encoding="utf-8") as f:
            data = json.load(f)
        if isinstance(data, dict):
            return data
    except Exception:
        pass
    return {}


def fetch(url):
    req = urllib.request.Request(url, headers={"User-Agent": "cii-skill"})
    with urllib.request.urlopen(req, timeout=20, context=_SSL_CTX) as r:
        return r.read().decode()


def _collect_names(node, out):
    """Recursively walk the criteria.yml nested structure.

    The file is a list of (tier_key, [sections]); each section is
    (section_name, [ (group, [...]) | (criterion_name, {attrs}) ]). A leaf
    criterion is a 2-tuple whose second element is a dict (it has 'category').
    """
    if isinstance(node, tuple) and len(node) == 2:
        key, val = node
        if isinstance(val, dict):
            out.append(key)          # leaf criterion
        elif isinstance(val, list):
            for child in val:
                _collect_names(child, out)
    elif isinstance(node, list):
        for child in node:
            _collect_names(child, out)


def fetch_section_map():
    """Return {section_arg: set(criterion_names)} for every known tier.

    Returns None if criteria.yml could not be fetched/parsed.
    """
    try:
        raw = fetch(CRITERIA_URL)
    except Exception as e:
        print(f"WARNING: could not fetch criteria definitions ({e}); "
              f"falling back to all-criteria behavior.", file=sys.stderr)
        return None
    try:
        doc = __import__("yaml").safe_load(raw)
    except Exception as e:
        print(f"WARNING: could not parse criteria definitions ({e}); "
              f"falling back to all-criteria behavior.", file=sys.stderr)
        return None

    # doc is a list of (tier_key, sections)
    tier_names = {}
    for item in doc:
        if isinstance(item, tuple) and len(item) == 2:
            tier_key, sections = item
            names = []
            _collect_names(sections, names)
            tier_names[tier_key] = set(names)

    section_map = {}
    for section_arg, tier_key in SECTION_TO_TIER.items():
        section_map[section_arg] = tier_names.get(tier_key, set())
    return section_map


def main():
    parser = argparse.ArgumentParser(
        description="List incomplete CII/OpenSSF Best Practices criteria.")
    parser.add_argument("project_id", nargs="?", default="14227")
    parser.add_argument("section", nargs="?", default="passing",
                        choices=list(SECTION_TO_TIER.keys()))
    parser.add_argument("--exclude", default="",
                        help="Comma-separated criterion names to skip in the URL.")
    args = parser.parse_args()

    project_id = args.project_id
    section = args.section
    exclude = {x.strip() for x in args.exclude.split(",") if x.strip()}

    justifications = load_justifications()

    data = json.loads(fetch(f"{BASE}/projects/{project_id}.json"))

    # All criteria reported by the project (every tier).
    criteria = {}
    for k, v in data.items():
        if k.endswith("_status") and isinstance(v, str):
            name = k[: -len("_status")]
            criteria[name] = {"status": v,
                              "justification": data.get(name + "_justification", "") or ""}

    # Build the section -> criterion-name set from upstream definitions.
    section_map = fetch_section_map()
    target_names = section_map.get(section) if section_map else None
    if target_names is not None and not target_names:
        print(f"WARNING: no criteria found for section '{section}' in "
              f"criteria.yml; falling back to all-criteria behavior.",
              file=sys.stderr)
        target_names = None  # fall back: no section restriction

    # Restrict to the requested section when we have a valid name set.
    if target_names is not None:
        scoped = {n: c for n, c in criteria.items() if n in target_names}
    else:
        scoped = criteria

    incomplete = {n: c for n, c in scoped.items() if c["status"] != "Met"}
    doable = {n: c for n, c in incomplete.items() if c["status"] in ("Unmet", "?")}
    na = {n: c for n, c in incomplete.items() if c["status"] == "N/A"}

    print(f"Project {project_id}: badge_level={data.get('badge_level')} "
          f"(passing%={data.get('badge_percentage_0')} "
          f"silver%={data.get('badge_percentage_1')} "
          f"gold%={data.get('badge_percentage_2')})")
    scope_note = f"section '{section}'" if target_names is not None else "ALL sections (fallback)"
    print(f"Scope: {scope_note} | criteria in scope: {len(scoped)} | "
          f"Incomplete: {len(incomplete)} "
          f"(Unmet/? : {len(doable)} | N/A: {len(na)})\n")

    print("Incomplete (Unmet/? — candidate to mark Met):")
    for n, c in sorted(doable.items()):
        print(f"  - {n}: {c['status']}  | current note: {c['justification'][:70]}")
    print("\nN/A (review separately — usually leave as N/A):")
    for n, c in sorted(na.items()):
        print(f"  - {n}: N/A | note: {c['justification'][:70]}")

    # Build URL params, restricted to the section AND honoring --exclude.
    # N/A criteria are never pre-filled as Met (they are excluded from `doable`).
    params = []
    for n in sorted(doable):
        if exclude and n in exclude:
            continue  # leave at current Unmet/?/N/A in the form
        params.append((f"{n}_status", "Met"))
        params.append((f"{n}_justification",
                       justifications.get(n, "TODO: justify")))
    params.append(("overrides", "*,osps_*"))
    edit_url = f"{BASE}/projects/{project_id}/{section}/edit?{urllib.parse.urlencode(params)}"
    print(f"\n=== Edit URL ({section}"
          + (f", excluding {sorted(exclude)}" if exclude else "")
          + f") ===\n{edit_url}")
    print(f"\nURL param count for _status/_justification pairs: "
          f"{sum(1 for k,_ in params if k.endswith('_status'))}")
    print("WARNING: pre-fills Unmet/? criteria as 'Met' with a placeholder justification.")
    print("Review EVERY field before Save; only mark Met what you genuinely satisfy.")


if __name__ == "__main__":
    main()
