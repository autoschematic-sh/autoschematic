/**
 * Turn a template containing [groupName] placeholders into a RegExp
 * whose groups can be read via `match(...).groups?.groupName`.
 *
 * Special regex metacharacters in the static parts are escaped automatically.
 * Each placeholder becomes   (?<groupName>[^/]+)
 *
 * Example:
 *   const rx = matchAddr("snowflake/warehouses/$[name].sql");
 *   const m  = "snowflake/warehouses/WH1.sql".match(rx);
 *   console.error(m?.groups?.name);   // -> "WH1"
 */
export const matchAddr = (template: string): RegExp => {
  // Escape any chars that may be interpreted as a regex
  const escape = (s: string) => s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');

  // Start of regex
  let rx = '^';

  for (let i = 0; i < template.length; i++) {
    // Look for "["...
    if (template[i] === '[') {
      i += 1;
      const start = i;
      // ...look for "]"
      while (i < template.length && template[i] !== ']') i++;
      // Check if we hit the end before we found "]"
      if (i === template.length) throw new Error('matchAddr: Unclosed [...] in pattern');

      // Extract the [groupName] from in-between the [ and ] ...
      const groupName = template.slice(start, i);
      // ... and form a regex from it!
      rx += `(?<${groupName}>[^/]+)`;
    } else {
      rx += escape(template[i]);
    }
  }

  // End of regex
  rx += '$';
  return new RegExp(rx);
};

export const formAddrObject = (matches: RegExpMatchArray, input: Object): Object => {
  const output = input;

  for (const [key, value] of Object.entries(matches.groups!)) {
    output[key] = value
  }

  return output;
};



/**
 * Replaces every bracketed token `[key]` in a template path with the
 * corresponding property from `addr`.
 *
 * @param template A path template containing `[key]` placeholders.
 * @param addr     An object whose keys supply the placeholder values.
 *
 * @example
 * formPath(
 *   "snowflake/databases/[database]/[schema]/external_tables/[name].sql",
 *   { database: "tea_types", schema: "public", name: "oolong" }
 * );
 * -> "snowflake/databases/tea_types/public/external_tables/oolong.sql"
 */
export function formPath(
  template: string,
  addr: Object
): string {
  return template.replace(/\[([^\]]+)]/g, (_, key: string) => {
    if (!(key in addr)) {
      throw new Error(`Missing value for placeholder [${key}]`);
    }
    return String(addr[key]);
  });
}
