export function pickReference() {
  const input = document.createElement("input");
  input.type = "file";
  input.accept = ".png,.jpg,.jpeg,.bmp,.gif";
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
      input.showPicker();
    } catch (error) {
      input.remove();
      reject(error);
    }
  });
}

export function downloadSvg(name, svg) {
  const url = URL.createObjectURL(new Blob([svg], { type: "image/svg+xml" }));
  const link = document.createElement("a");
  link.href = url;
  link.download = name;
  document.body.append(link);
  link.click();
  link.remove();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}
