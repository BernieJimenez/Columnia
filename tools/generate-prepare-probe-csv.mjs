// Writes a synthetic CSV for the native Preparar flow probe: padded text,
// empty values, "N/A" markers and exact duplicate rows. No real data.
// Usage: node tools/generate-prepare-probe-csv.mjs <output.csv> <rows>
import fs from "node:fs";
const out = fs.createWriteStream(process.argv[2]);
const rows = Number(process.argv[3]);
const cities = [" Santiago ", "Santiago", "La Vega", "Santo Domingo", "", "Puerto Plata  "];
const states = ["activo", "inactivo", "N/A", "pendiente"];
out.write("id,ciudad,monto,estado,fecha,correo\n");
let buffer = "";
for (let i = 0; i < rows; i++) {
  const monto = i % 9 === 0 ? "" : String((i * 37) % 5000);
  const line = `${i},${cities[i % cities.length]},${monto},${states[i % states.length]},2026-0${(i % 9) + 1}-1${i % 10},usuario${i % 50000}@example.com\n`;
  buffer += line;
  if (i % 997 === 0) buffer += line; // exact duplicate
  if (buffer.length > 1 << 20) { out.write(buffer); buffer = ""; }
}
out.write(buffer);
out.end(() => console.log("ok", fs.statSync(process.argv[2]).size));
