/** Configuration Tailwind de LexProcedura (feuille compilée : ui/css/app.css). */
module.exports = {
  content: ['../../ui/index.html', '../../ui/renderer.js'],
  theme: {
    extend: {
      fontFamily: { sans: ['Inter', 'Segoe UI', 'system-ui', 'sans-serif'] },
      colors: { brand: { 50: '#f8fafc', 600: '#0284c7', 700: '#0369a1', 900: '#0f172a' } },
      boxShadow: {
        '2xs': '0 1px 0 0 rgb(0 0 0 / 0.04)',
        xs: '0 1px 2px 0 rgb(0 0 0 / 0.05)',
      },
      backdropBlur: { xs: '2px' },
    },
  },
  plugins: [],
};
