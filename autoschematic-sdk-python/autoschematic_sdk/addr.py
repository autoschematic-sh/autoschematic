from __future__ import annotations

from dataclasses import dataclass
import re


def match_addr(template: str) -> re.Pattern[str]:
    """Turn a template containing ``[groupName]`` placeholders into a compiled
    regex whose named groups can be read via ``m.group("groupName")``.

    Each ``[placeholder]`` becomes ``(?P<placeholder>[^/]+)``.
    Static parts are regex-escaped automatically.

    Example::

        rx = match_addr("snowflake/warehouses/[name].sql")
        m  = rx.match("snowflake/warehouses/WH1.sql")
        assert m and m.group("name") == "WH1"
    """
    pattern = "^"
    i = 0
    while i < len(template):
        if template[i] == "[":
            i += 1
            start = i
            while i < len(template) and template[i] != "]":
                i += 1
            if i == len(template):
                raise ValueError("match_addr: Unclosed [...] in pattern")
            group_name = template[start:i]
            pattern += f"(?P<{group_name}>[^/]+)"
            i += 1
        else:
            pattern += re.escape(template[i])
            i += 1
    pattern += "$"
    return re.compile(pattern)


def form_addr_object(match: re.Match[str], input: dict[str, str]) -> dict[str, str]:
    """Merge named groups from a regex match into *input* and return it."""
    output = dict(input)
    groups = match.groupdict()
    output.update(groups)
    return output


def form_path(template: str, addr: dict[str, str]) -> str:
    """Replace every ``[key]`` token in *template* with the value from *addr*.

    Example::

        form_path(
            "snowflake/databases/[database]/[schema]/external_tables/[name].sql",
            {"database": "tea_types", "schema": "public", "name": "oolong"},
        )
        # -> "snowflake/databases/tea_types/public/external_tables/oolong.sql"
    """
    def _replace(m: re.Match[str]) -> str:
        key = m.group(1)
        if key not in addr:
            raise KeyError(f"Missing value for placeholder [{key}]")
        return str(addr[key])

    return re.sub(r"\[([^\]]+)\]", _replace, template)

@dataclass
class PointerToVirtual:
    addr: str

@dataclass
class OutputMap:
    map: dict