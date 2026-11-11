// Compile and render Registry content using the installed website toolchain.
// Usage: node render-docs.mjs /path/to/agentpm-api /path/to/agentpm-web
import { readFileSync, readdirSync } from "node:fs";
import { createRequire } from "node:module";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";

if (process.argv.length !== 4) {
  throw new Error("Usage: node render-docs.mjs <registry-root> <web-root>");
}
const docs = resolve(process.argv[2], "docs");
const requireFromWeb = createRequire(resolve(process.argv[3], "package.json"));
const load = (name) => import(pathToFileURL(requireFromWeb.resolve(name)).href);
const [mdx, jsx, react, server, gfm, directives, slug, anchors, pretty, util] =
  await Promise.all([
    load("@mdx-js/mdx"),
    load("react/jsx-runtime"),
    load("react"),
    load("react-dom/server"),
    load("remark-gfm"),
    load("remark-directive"),
    load("rehype-slug"),
    load("rehype-autolink-headings"),
    load("rehype-pretty-code"),
    load("unist-util-visit"),
  ]);
const React = react.default;
function calloutDirectives() {
  return (tree) => {
    util.visit(tree, (node) => {
      if (
        ["containerDirective", "textDirective", "leafDirective"].includes(
          node.type,
        ) &&
        ["tip", "info", "warn", "danger"].includes(node.name)
      ) {
        const type = node.name;
        node.type =
          node.type === "textDirective"
            ? "mdxJsxTextElement"
            : "mdxJsxFlowElement";
        node.name = "Callout";
        node.attributes = [
          ...(node.attributes || []),
          { type: "mdxJsxAttribute", name: "type", value: type },
        ];
        if (node.label)
          node.attributes.push({
            type: "mdxJsxAttribute",
            name: "title",
            value: node.label,
          });
      }
    });
  };
}
const files = readdirSync(docs, { recursive: true }).filter((name) =>
  name.endsWith(".mdx"),
);
for (const file of files) {
  const content = readFileSync(resolve(docs, file), "utf8").replace(
    /^---\r?\n[\s\S]*?\r?\n---\r?\n/,
    "",
  );
  try {
    const { default: Content } = await mdx.evaluate(content, {
      ...jsx,
      remarkPlugins: [gfm.default, directives.default, calloutDirectives],
      rehypePlugins: [
        slug.default,
        [anchors.default, { behavior: "append" }],
        [
          pretty.default,
          { theme: "github-dark", keepBackground: false, defaultLang: "text" },
        ],
      ],
    });
    const Callout = ({ children }) =>
      React.createElement("aside", null, children);
    const html = server.renderToStaticMarkup(
      React.createElement(Content, { components: { Callout } }),
    );
    if (!html.trim()) throw new Error("Empty documentation render");
    if (/(?:Milestone\s*\d|Stage\s*\d|Release Band\s*\d)/i.test(html)) {
      throw new Error("Internal planning label in rendered documentation");
    }
  } catch (error) {
    throw new Error(`Cannot render ${file}: ${error.message}`, {
      cause: error,
    });
  }
}
console.log(`PASS: ${files.length} documentation pages compile and render.`);
