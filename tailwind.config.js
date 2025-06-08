/** @type {import('tailwindcss').Config} */
/*
    * run this 
    * npx tailwindcss -i ./style/tailwind.css -o ./style/output.css --watch
    * */
module.exports = {
  content: {
    files: ["*.html", "./src/**/*.rs"],
  },
  darkMode: "class",
  theme: {
    colors: {
      transparent: 'transparent',
      current: 'currentColor',
      'white': '#ffffff',
      'black': '#000000',

      'gray': {
        DEFAULT: '#DCE9E6',
        900: '#233833',
        800: '#467066',
        700: '#6da497',
        600: '#a5c6bf',
        500: '#dce9e6',
        400: '#e3eeeb',
        300: '#eaf2f0',
        200: '#f1f6f5',
        100: '#f8fbfa'
      },

      'teal': {
        DEFAULT: '#042F2E',
        900: '#010909',
        800: '#021312',
        700: '#021c1c',
        600: '#032625',
        500: '#042f2e',
        400: '#0b8381',
        300: '#13d8d5',
        200: '#56f0ee',
        100: '#abf8f6'
      },

      'mint': {
        DEFAULT: '#CCFBF1',
        900: '#075443',
        800: '#0ea887',
        700: '#23edc1',
        600: '#77f4d9',
        500: '#ccfbf1',
        400: '#d5fcf3',
        300: '#e0fcf6',
        200: '#eafdf9',
        100: '#f5fefc'
      },

      'seafoam': {
        DEFAULT: '#206D5F',
        900: '#061613',
        800: '#0d2c27',
        700: '#13423a',
        600: '#1a594d',
        500: '#206d5f',
        400: '#31a892',
        300: '#54cdb7',
        200: '#8ddecf',
        100: '#c6eee7'
      },

      'wenge': {
        DEFAULT: '#715F58',
        900: '#161312',
        800: '#2d2623',
        700: '#433935',
        600: '#594b46',
        500: '#715f58',
        400: '#917c74',
        300: '#ad9d96',
        200: '#c8beb9',
        100: '#e4dedc'
      },

      'aqua': {
        DEFAULT: '#00AAA8',
        900: '#002221',
        800: '#004342',
        700: '#006563',
        600: '#008784',
        500: '#00aaa8',
        400: '#00ede9',
        300: '#32fffc',
        200: '#76fffd',
        100: '#bbfffe'
      },

      'salmon': {
        DEFAULT: '#FDA4AF',
        900: '#52020b',
        800: '#a40316',
        700: '#f60521',
        600: '#fb5367',
        500: '#fda4af',
        400: '#feb7bf',
        300: '#fec9cf',
        200: '#fedbdf',
        100: '#ffedef'
      },
      
      // Custom colors for similarity indicators
      'aquamarine': {
        DEFAULT: '#2dd4bf',
        light: '#99f6e4',
        dark: '#0d9488'
      },
      'purple': {
        DEFAULT: '#a78bfa',
        light: '#c4b5fd',
        dark: '#7c3aed'
      },
      'orange': {
        DEFAULT: '#fb923c',
        light: '#fdba74',
        dark: '#ea580c'
      },
    },
    extend: {
      height: {
        '108': '26rem',
        '128': '32rem',
        '172': '64rem',
      }
    },
  },
  plugins: [],
}
