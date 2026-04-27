export type OcrLevel = 'word' | 'line' | 'block' | 'paragraph';

export interface OcrTextBlock {
  text: string;
  bbox?: [number, number, number, number];
  polygon?: [number, number][];
  confidence?: number;
  fontSize?: number;
  level: OcrLevel;
}

export interface OcrResult {
  blocks: OcrTextBlock[];
  fullText: string;
  language?: string;
  engineId: string;
  latencyMs: number;
}

/** Type guard: is this a line-level block? */
export function isLineBlock(block: OcrTextBlock): boolean {
  return block.level === 'line';
}

/** Type guard: is this a word-level block? */
export function isWordBlock(block: OcrTextBlock): boolean {
  return block.level === 'word';
}

/** Filter result to only line blocks (for layout overlay) */
export function getLineBlocks(result: OcrResult): OcrTextBlock[] {
  return result.blocks.filter(isLineBlock);
}
