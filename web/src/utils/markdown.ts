import type { MarkdownContent, MarkdownFrontmatter } from '@/types/markdown';

export function parseMarkdown(source: string, slug: string, validate = false): MarkdownContent {
  const frontmatterRegex = /^---\s*\n([\s\S]*?)\n---\s*\n([\s\S]*)$/;
  const match = source.match(frontmatterRegex);

  if (!match) {
    if (validate) {
      throw new Error(`Markdown file "${slug}" missing frontmatter.`);
    }
    return {
      frontmatter: {
        title: slug,
        description: '',
        keywords: '',
        author: '',
        date: '',
        image: '',
        slug,
      },
      content: source.trim(),
      slug,
    };
  }

  const [, frontmatterText, content] = match;
  const data: Record<string, string> = {};

  frontmatterText.split('\n').forEach(line => {
    const colonIndex = line.indexOf(':');
    if (colonIndex === -1) return;

    const key = line.slice(0, colonIndex).trim();
    const value = line.slice(colonIndex + 1).trim().replace(/^['"]|['"]$/g, '');

    if (key) {
      data[key] = value;
    }
  });

  if (validate) {
    if (!data.title) throw new Error(`Markdown file "${slug}" missing required field: title`);
    if (!data.description) throw new Error(`Markdown file "${slug}" missing required field: description`);
    if (!data.author) throw new Error(`Markdown file "${slug}" missing required field: author`);
    if (!data.date) throw new Error(`Markdown file "${slug}" missing required field: date`);
  }

  const frontmatter: MarkdownFrontmatter = {
    title: data.title || slug,
    description: data.description || '',
    keywords: data.keywords || '',
    author: data.author || '',
    date: data.date || '',
    image: data.image || '',
    slug: data.slug || slug,
  };

  return {
    frontmatter,
    content: content.trim(),
    slug,
  };
}

export function getPageTitle(title: string): string {
  return `${title} | SystemPrompt`;
}

export function generateSlug(filename: string): string {
  return filename
    .toLowerCase()
    .replace(/\.md$/, '')
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-|-$/g, '');
}
