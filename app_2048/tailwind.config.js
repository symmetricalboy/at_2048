/** @type {import('tailwindcss').Config} */
module.exports = {
  content: [
    "./src/**/*.rs",
    "./index.html",
  ],
  theme: {
    extend: {},
  },
  plugins: [
    require('daisyui'),
  ],
  daisyui: {
    themes: [], // Puedes especificar temas aquí, ej: ["light", "dark", "cupcake"]
    styled: true,
    base: true,
    utils: true,
    logs: true,
    rtl: false,
  }
} 