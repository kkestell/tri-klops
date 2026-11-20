export function pickReference() {
  const input = document.createElement("input");
  input.type = "file";
  input.accept = ".png,.jpg,.jpeg,.bmp,.gif,.webp";
  input.hidden = true;
  document.body.append(input);
  return new Promise((resolve, reject) => {
    const finish = (file) => {
      input.remove();
      resolve(file);
    };
    input.addEventListener("change", () => finish(input.files[0]), { once: true });
    input.addEventListener("cancel", () => finish(null), { once: true });
    try {
      input.click();
    } catch (error) {
      input.remove();
      reject(error);
    }
  });
}

export async function renderJpg(svg) {
  const drawing = new DOMParser().parseFromString(svg, "image/svg+xml").documentElement;
  const width = Number(drawing.getAttribute("width"));
  const height = Number(drawing.getAttribute("height"));
  const scale = 2048 / Math.max(width, height);
  const canvas = document.createElement("canvas");
  canvas.width = Math.max(1, Math.round(width * scale));
  canvas.height = Math.max(1, Math.round(height * scale));

  // Set the intrinsic size so the browser rasterizes the vectors at export resolution.
  drawing.setAttribute("width", canvas.width);
  drawing.setAttribute("height", canvas.height);
  const markup = new XMLSerializer().serializeToString(drawing);
  const url = URL.createObjectURL(new Blob([markup], { type: "image/svg+xml" }));
  try {
    const image = new Image();
    image.src = url;
    await image.decode();
    const context = canvas.getContext("2d", { alpha: false });
    if (!context) throw new Error("Could not create the export canvas.");
    context.drawImage(image, 0, 0);
  } finally {
    URL.revokeObjectURL(url);
  }

  const jpg = await new Promise((resolve) => canvas.toBlob(resolve, "image/jpeg", 0.95));
  if (!jpg) throw new Error("Could not encode the JPG.");
  return jpg;
}

export function downloadFile(name, type, contents) {
  const url = URL.createObjectURL(new Blob([contents], { type }));
  const link = document.createElement("a");
  link.href = url;
  link.download = name;
  document.body.append(link);
  link.click();
  link.remove();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}
