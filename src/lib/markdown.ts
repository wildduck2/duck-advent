import { marked } from "marked";
import { markedTerminal } from "marked-terminal";

marked.use(
  markedTerminal({
    width: 100,
    reflowText: true,
    tab: 2,
  }) as Parameters<typeof marked.use>[0],
);

export function renderMarkdown(source: string): string {
  return marked.parse(source) as string;
}
