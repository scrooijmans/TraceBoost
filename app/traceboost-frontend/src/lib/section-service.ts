import type { SectionAxis, SectionView } from "@traceboost/seis-contracts";

export async function fetchSectionView(axis: SectionAxis, index: number): Promise<SectionView> {
  const response = await fetch(`/api/section?axis=${encodeURIComponent(axis)}&index=${index}`);
  if (!response.ok) {
    const text = await response.text();
    throw new Error(text || "Failed to load section");
  }
  return response.json() as Promise<SectionView>;
}
