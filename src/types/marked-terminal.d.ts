declare module "marked-terminal" {
  import type { MarkedExtension } from "marked";
  interface Options {
    code?: unknown;
    blockquote?: unknown;
    html?: unknown;
    heading?: unknown;
    firstHeading?: unknown;
    hr?: unknown;
    listitem?: unknown;
    list?: unknown;
    table?: unknown;
    paragraph?: unknown;
    strong?: unknown;
    em?: unknown;
    codespan?: unknown;
    del?: unknown;
    link?: unknown;
    href?: unknown;
    text?: unknown;
    unescape?: boolean;
    emoji?: boolean;
    width?: number;
    showSectionPrefix?: boolean;
    reflowText?: boolean;
    tab?: number;
    tableOptions?: unknown;
  }
  export function markedTerminal(options?: Options): MarkedExtension;
  const _default: typeof markedTerminal;
  export default _default;
}
