/** @type {import('tailwindcss').Config} */
export default {
	content: ["./index.html", "./src/**/*.{svelte,js,ts,jsx,tsx}"],
	darkMode: "class",
	theme: {
		extend: {
			boxShadow: {
				bottom: "outset 0 2px 0 var(--tw-shadow-colored)",
			},
		},
	},
	plugins: [require("flowbite-typography"), require("flowbite/plugin")],
};
