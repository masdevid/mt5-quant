#!/usr/bin/env python3
"""List incomplete OpenSSF Best Practices criteria and generate a pre-filled edit URL.

Usage:
    cii_criteria.py <project_id> [section]
                    [--schema {osps,metal}] [--exclude n1,n2,n3]
                    [--na n1,n2,n3] [--no-open] [--open]

    project_id: bestpractices.dev project numeric id (default: 14227)
    section:    passing | silver | gold | baseline-1 | baseline-2 | baseline-3
                (default: passing for --schema metal; baseline-1 for --schema osps)
    --schema:   osps  (DEFAULT) -> OSPS Baseline schema (baseline_criteria.yml;
                           project JSON uses `OSPS-*` criterion ids)
                metal (legacy)  -> "Metal" schema (criteria.yml; short criterion
                           names like `static_analysis`). Keeps the old behavior.
    --exclude:  comma-separated criterion ids to SKIP from the pre-filled URL
                (they stay at their current Unmet/?/N/A in the form).
     --na:      comma-separated criterion ids to mark as N/A in the pre-filled URL
                (emits <id>_status=N/A with the justification from the JSON).
     --no-open: never open the generated URL in a browser (useful for CI/piping).
     --open:    force-open the URL even when stdout is not a TTY (e.g. when piped).

When run interactively (stdout is a TTY) the script auto-opens the generated
edit URL in the default browser. Piping to a file or another command prints the
URL only (no browser spawn). Use --no-open to suppress auto-open, --open to
force it regardless of TTY.

SCHEMA DIFFERENCES
------------------
* metal  — fetches criteria.yml, maps tiers via its nested structure, and
  pre-fills every Unmet/? criterion in the section as `Met` (placeholder
  justification when none is known). This is the legacy behavior.
* osps   — fetches baseline_criteria.yml, maps `original_id` -> tier, and only
  pre-fills a criterion as `Met` when a truthful justification exists in
  cii_justifications.json (keyed by the `OSPS-*` id). Criteria without a
  justification are left at their current Unmet/? state so the URL never claims
  more than is documented. `overrides=*,osps_*` is always appended.
"""
import sys, os, json, subprocess, urllib.request, urllib.parse, argparse
import ssl

try:
    import certifi
    _SSL_CTX = ssl.create_default_context(cafile=certifi.where())
except Exception:
    _SSL_CTX = ssl.create_default_context()

BASE = "https://www.bestpractices.dev"

# Legacy "Metal" schema definitions.
METAL_CRITERIA_URL = ("https://raw.githubusercontent.com/ossf/best-practices-badge/"
                      "main/criteria/criteria.yml")

# OSPS Baseline schema definitions.
OSPS_CRITERIA_URL = ("https://raw.githubusercontent.com/ossf/best-practices-badge/"
                     "main/criteria/baseline_criteria.yml")

OSPS_TIERS = ("baseline-1", "baseline-2", "baseline-3")

# CLI section argument -> tier key inside the legacy criteria.yml.
METAL_SECTION_TO_TIER = {
    "passing": "0",
    "silver": "1",
    "gold": "2",
    "baseline-1": "baseline-1",
    "baseline-2": "baseline-2",
    "baseline-3": "baseline-3",
}

# Default section per schema (used only when no section argument is given).
SCHEMA_DEFAULT_SECTION = {
    "metal": "passing",
    "osps": "baseline-1",
}

# Criterion ids whose status is pre-fillable as "Met".
DOABLE_STATUSES = ("Unmet", "?")


def load_justifications():
    """Load real justifications from the sibling cii_justifications.json.

    Returns a dict id -> justification string. Falls back to {} if the file
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


def open_url(url):
    """Open `url` in the default browser for the current platform.

    Fails silently with a clear fallback message if the opener is unavailable
    or errors — never raises, so the script always exits cleanly.
    """
    if sys.platform == "darwin":
        cmd = ["open", url]
    elif sys.platform.startswith("win"):
        # Windows: `start` needs a dummy title before the URL argument.
        cmd = ["cmd", "/c", "start", "", url]
    else:
        cmd = ["xdg-open", url]

    try:
        subprocess.run(cmd, check=True)
    except Exception as e:
        print(f"\nCould not auto-open the browser ({e}).\n"
              f"Open this URL manually:\n{url}", file=sys.stderr)


def load_project_criteria(project_id):
    """Fetch the project JSON and return {criterion_id: {status, justification}}."""
    data = json.loads(fetch(f"{BASE}/projects/{project_id}.json"))
    criteria = {}
    for k, v in data.items():
        if k.endswith("_status") and isinstance(v, str):
            name = k[: -len("_status")]
            criteria[name] = {
                "status": v,
                "justification": data.get(name + "_justification", "") or "",
            }
    return data, criteria


# --------------------------------------------------------------------------- #
# Metal (legacy) schema                                                        #
# --------------------------------------------------------------------------- #

def _collect_metal_names(node, out):
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
                _collect_metal_names(child, out)
    elif isinstance(node, list):
        for child in node:
            _collect_metal_names(child, out)


def fetch_metal_section_map():
    """Return {section_arg: set(criterion_names)} for every known Metal tier.

    Returns None if criteria.yml could not be fetched/parsed.
    """
    try:
        raw = fetch(METAL_CRITERIA_URL)
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
            _collect_metal_names(sections, names)
            tier_names[tier_key] = set(names)

    section_map = {}
    for section_arg, tier_key in METAL_SECTION_TO_TIER.items():
        section_map[section_arg] = tier_names.get(tier_key, set())
    return section_map


def run_metal(project_id, section, exclude, na_set, justifications, args):
    data, criteria = load_project_criteria(project_id)

    section_map = fetch_metal_section_map()
    target_names = section_map.get(section) if section_map else None
    if target_names is not None and not target_names:
        print(f"WARNING: no criteria found for section '{section}' in "
              f"criteria.yml; falling back to all-criteria behavior.",
              file=sys.stderr)
        target_names = None  # fall back: no section restriction

    if target_names is not None:
        scoped = {n: c for n, c in criteria.items() if n in target_names}
    else:
        scoped = criteria

    incomplete = {n: c for n, c in scoped.items() if c["status"] != "Met"}
    doable = {n: c for n, c in incomplete.items() if c["status"] in DOABLE_STATUSES}
    na = {n: c for n, c in incomplete.items() if c["status"] == "N/A"}

    print(f"Project {project_id}: badge_level={data.get('badge_level')} "
          f"(passing%={data.get('badge_percentage_0')} "
          f"silver%={data.get('badge_percentage_1')} "
          f"gold%={data.get('badge_percentage_2')})")
    scope_note = (f"section '{section}'" if target_names is not None
                  else "ALL sections (fallback)")
    print(f"Schema: metal | Scope: {scope_note} | "
          f"criteria in scope: {len(scoped)} | "
          f"Incomplete: {len(incomplete)} "
          f"(Unmet/? : {len(doable)} | N/A: {len(na)})\n")

    print("Incomplete (Unmet/? — candidate to mark Met):")
    for n, c in sorted(doable.items()):
        print(f"  - {n}: {c['status']}  | current note: {c['justification'][:70]}")
    print("\nN/A (review separately — usually leave as N/A):")
    for n, c in sorted(na.items()):
        print(f"  - {n}: N/A | note: {c['justification'][:70]}")

    params = []
    for n in sorted(doable):
        if n in exclude:
            continue  # leave at current Unmet/?/N/A in the form
        if n in na_set:
            continue  # emitted as N/A below instead of Met
        params.append((f"{n}_status", "Met"))
        params.append((f"{n}_justification",
                       justifications.get(n, "TODO: justify")))
    for n in sorted(na_set):
        if n in exclude:
            continue
        if n not in scoped:
            continue
        params.append((f"{n}_status", "N/A"))
        params.append((f"{n}_justification",
                       justifications.get(n, "TODO: justify N/A")))
    params.append(("overrides", "*,osps_*"))
    edit_url = (f"{BASE}/projects/{project_id}/{section}/edit?"
                f"{urllib.parse.urlencode(params)}")
    print(f"\n=== Edit URL ({section}"
          + (f", excluding {sorted(exclude)}" if exclude else "")
          + (f", N/A {sorted(na_set)}" if na_set else "")
          + f") ===\n{edit_url}")
    print(f"\nURL param count for _status/_justification pairs: "
          f"{sum(1 for k, _ in params if k.endswith('_status'))}")
    print("WARNING: pre-fills Unmet/? criteria as 'Met' (and --na as 'N/A') using "
          "justifications from cii_justifications.json.")
    print("Review EVERY field before Save; only mark Met what you genuinely satisfy.")
    return edit_url


# --------------------------------------------------------------------------- #
# OSPS Baseline schema                                                         #
# --------------------------------------------------------------------------- #

def _collect_osps_controls(node, out):
    """Recursively collect every control definition under a node.

    A control is any dict containing an `original_id` key. We map that id to its
    {category, na_allowed, na_justification_required} so callers can scope by
    tier and validate N/A eligibility.
    """
    if isinstance(node, dict):
        if "original_id" in node:
            out[node["original_id"]] = {
                "category": node.get("category"),
                "na_allowed": bool(node.get("na_allowed")),
                "na_justification_required": bool(
                    node.get("na_justification_required")),
            }
        else:
            for v in node.values():
                _collect_osps_controls(v, out)
    elif isinstance(node, list):
        for v in node:
            _collect_osps_controls(v, out)


def fetch_osps_tier_map():
    """Return {tier: {original_id: control_meta}} from baseline_criteria.yml.

    Returns {} if the file could not be fetched/parsed (caller falls back to
    all-criteria behavior).
    """
    try:
        raw = fetch(OSPS_CRITERIA_URL)
    except Exception as e:
        print(f"WARNING: could not fetch OSPS criteria definitions ({e}); "
              f"falling back to all-criteria behavior.", file=sys.stderr)
        return {}
    try:
        doc = __import__("yaml").safe_load(raw)
    except Exception as e:
        print(f"WARNING: could not parse OSPS criteria definitions ({e}); "
              f"falling back to all-criteria behavior.", file=sys.stderr)
        return {}

    if not isinstance(doc, dict):
        return {}

    tier_map = {}
    for tier in OSPS_TIERS:
        controls = {}
        if tier in doc:
            _collect_osps_controls(doc[tier], controls)
        tier_map[tier] = controls
    return tier_map


def run_osps(project_id, section, exclude, na_set, justifications, args):
    data, criteria = load_project_criteria(project_id)

    tier_map = fetch_osps_tier_map()
    controls = tier_map.get(section)
    if controls is None:
        print(f"WARNING: section '{section}' is not an OSPS Baseline tier; "
              f"falling back to all-criteria behavior.", file=sys.stderr)
        target_ids = None
    elif not controls:
        print(f"WARNING: no OSPS criteria found for tier '{section}'; "
              f"falling back to all-criteria behavior.", file=sys.stderr)
        target_ids = None
    else:
        target_ids = set(controls.keys())

    if target_ids is not None:
        scoped = {n: c for n, c in criteria.items() if n in target_ids}
    else:
        scoped = criteria

    incomplete = {n: c for n, c in scoped.items() if c["status"] != "Met"}
    doable = {n: c for n, c in incomplete.items() if c["status"] in DOABLE_STATUSES}
    na = {n: c for n, c in incomplete.items() if c["status"] == "N/A"}

    print(f"Project {project_id}: badge_level={data.get('badge_level')}")
    scope_note = (f"tier '{section}'" if target_ids is not None
                  else "ALL criteria (fallback)")
    print(f"Schema: osps | Scope: {scope_note} | "
          f"criteria in scope: {len(scoped)} | "
          f"Incomplete: {len(incomplete)} "
          f"(Unmet/? : {len(doable)} | N/A: {len(na)})\n")

    print("Incomplete (Unmet/? — only those with a justification are pre-filled "
          "as Met):")
    for n, c in sorted(doable.items()):
        have = "J" if n in justifications else " "
        print(f"  [{have}] {n}: {c['status']}  | current note: "
              f"{c['justification'][:70]}")
    print("\nN/A (review separately — usually leave as N/A):")
    for n, c in sorted(na.items()):
        print(f"  - {n}: N/A | note: {c['justification'][:70]}")

    # Pre-fill Met ONLY for doable criteria that have a truthful justification
    # recorded locally. Criteria without one are left at their current state so
    # the URL never over-claims.
    params = []
    prefilled_met = []
    for n in sorted(doable):
        if n in exclude:
            continue
        if n in na_set:
            continue  # handled below as N/A
        justification = justifications.get(n)
        if not justification:
            continue
        params.append((f"{n}_status", "Met"))
        params.append((f"{n}_justification", justification))
        prefilled_met.append(n)

    # Pre-fill N/A for --na criteria that are in scope and allow N/A.
    for n in sorted(na_set):
        if n in exclude:
            continue
        if n not in scoped:
            continue
        ctrl = controls.get(n) if controls else None
        if ctrl is not None and not ctrl["na_allowed"]:
            print(f"WARNING: {n} does not allow N/A (na_allowed=false); "
                  f"skipping --na for it.", file=sys.stderr)
            continue
        justification = justifications.get(
            n, criteria.get(n, {}).get("justification") or "TODO: justify N/A")
        params.append((f"{n}_status", "N/A"))
        params.append((f"{n}_justification", justification))

    params.append(("overrides", "*,osps_*"))
    edit_url = (f"{BASE}/projects/{project_id}/{section}/edit?"
                f"{urllib.parse.urlencode(params)}")
    print(f"\n=== Edit URL ({section}"
          + (f", excluding {sorted(exclude)}" if exclude else "")
          + (f", N/A {sorted(na_set)}" if na_set else "")
          + f") ===\n{edit_url}")
    print(f"\nPre-filled Met ({len(prefilled_met)}): {sorted(prefilled_met)}")
    print(f"URL param count for _status/_justification pairs: "
          f"{sum(1 for k, _ in params if k.endswith('_status'))}")
    print("WARNING: pre-fills only criteria with a recorded justification as "
          "'Met' (and --na as 'N/A').")
    print("Review EVERY field before Save; only mark Met what you genuinely satisfy.")
    return edit_url


# --------------------------------------------------------------------------- #
# Entry point                                                                  #
# --------------------------------------------------------------------------- #

def main():
    parser = argparse.ArgumentParser(
        description="List incomplete OpenSSF Best Practices (CII) criteria and "
                    "generate a pre-filled edit URL. Defaults to the OSPS "
                    "Baseline schema.")
    parser.add_argument("project_id", nargs="?", default="14227")
    parser.add_argument("section", nargs="?", default=None,
                        choices=list(METAL_SECTION_TO_TIER.keys()))
    parser.add_argument("--schema", default="osps", choices=["osps", "metal"],
                        help="Criteria schema: 'osps' (DEFAULT, OSPS Baseline) "
                             "or 'metal' (legacy).")
    parser.add_argument("--exclude", default="",
                        help="Comma-separated criterion ids to skip in the URL.")
    parser.add_argument("--na", default="",
                        help="Comma-separated criterion ids to mark as N/A "
                             "with the justification from cii_justifications.json.")
    parser.add_argument("--no-open", action="store_true",
                        help="Never open the generated URL in a browser "
                             "(useful for CI/piping).")
    parser.add_argument("--open", action="store_true",
                        help="Force-open the URL even when stdout is not a TTY.")
    args = parser.parse_args()

    project_id = args.project_id
    # Resolve the section: explicit arg wins; otherwise use the schema default.
    section = args.section or SCHEMA_DEFAULT_SECTION[args.schema]
    exclude = {x.strip() for x in args.exclude.split(",") if x.strip()}
    na_set = {x.strip() for x in args.na.split(",") if x.strip()}

    justifications = load_justifications()

    if args.schema == "metal":
        edit_url = run_metal(project_id, section, exclude, na_set,
                             justifications, args)
    else:
        edit_url = run_osps(project_id, section, exclude, na_set,
                            justifications, args)

    # Auto-open the URL in a browser. Default: only when run interactively
    # (stdout is a TTY) and --no-open was not given. --open forces it even when
    # piped; --no-open always disables it.
    if args.open or (sys.stdout.isatty() and not args.no_open):
        open_url(edit_url)


if __name__ == "__main__":
    main()
