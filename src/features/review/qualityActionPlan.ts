export type QualityActionTarget = "missingValues" | "duplicates" | "incompatibleTypes";

export interface QualityActionCounts {
  nullCount: number;
  nullColumnCount: number;
  duplicateCount: number;
  invalidTypeCount: number;
}

export interface QualityActionRecommendation {
  target: QualityActionTarget;
  title: string;
  explanation: string;
  impact: string;
  actionLabel: string;
}

export function qualityActionTargetDomId(target: QualityActionTarget): string {
  return `prepare-quality-${target}`;
}

export function buildQualityActionPlan(counts: QualityActionCounts): QualityActionRecommendation[] {
  const plan: QualityActionRecommendation[] = [];
  if (counts.nullCount > 0) {
    plan.push({
      target: "missingValues",
      title: "Valores sin dato",
      explanation: `${counts.nullCount.toLocaleString()} celdas sin valor en ${counts.nullColumnCount.toLocaleString()} ${counts.nullColumnCount === 1 ? "columna" : "columnas"}. Un nulo puede ser válido; primero revisa por qué falta antes de completarlo o retirar una columna.`,
      impact: "Rellenar puede cambiar la interpretación del dataset. Columnia conserva los nulos hasta que elijas una corrección reversible.",
      actionLabel: "Revisar opciones para nulos",
    });
  }
  if (counts.duplicateCount > 0) {
    plan.push({
      target: "duplicates",
      title: "Filas duplicadas exactas",
      explanation: `${counts.duplicateCount.toLocaleString()} filas adicionales coinciden con otra fila en todas las columnas.`,
      impact: "Retirarlas puede evitar doble conteo; confirma que las repeticiones no representen eventos distintos. La primera fila se conserva y el cambio se puede deshacer.",
      actionLabel: "Revisar duplicados exactos",
    });
  }
  if (counts.invalidTypeCount > 0) {
    plan.push({
      target: "incompatibleTypes",
      title: "Valores incompatibles con el tipo sugerido",
      explanation: `${counts.invalidTypeCount.toLocaleString()} celdas no coinciden con un tipo sugerido por el perfil.`,
      impact: "Apartar valores como nulos puede ocultar códigos o excepciones válidas. Revisa la sugerencia y su confirmación antes de aplicar el cambio reversible.",
      actionLabel: "Revisar tipos incompatibles",
    });
  }
  return plan;
}
