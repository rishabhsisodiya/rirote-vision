/** Every word typed must appear somewhere in the texts, in any order, ignoring case. */
export function matches(query: string, ...texts: string[]): boolean {
  const haystack = texts.join(" ").toLowerCase();
  return query
    .toLowerCase()
    .split(/\s+/)
    .filter(Boolean)
    .every((word) => haystack.includes(word));
}
