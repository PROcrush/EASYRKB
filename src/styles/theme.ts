/**
 * Runtime colours used by canvas rendering and mock data.
 *
 * The full measured design source is maintained privately. Keep this small
 * public map in sync with the shipped CSS tokens when a runtime consumer needs
 * a colour value rather than a CSS custom property.
 */
const theme = {
  color: {
    waveLow: { value: "#0055E1" }, waveMid: { value: "#FFA600" }, waveHigh: { value: "#FFFFFF" },
    waveLowMid: { value: "#B4690A" }, waveLowHigh: { value: "#D2DCFA" }, waveMidHigh: { value: "#FFF0D7" },
    waveAll: { value: "#F5EBD7" }, pcmBlue: { value: "#5BA0FF" }, pcm3Band: { value: "#FFA600" },
    pcmRgb: { value: "#F266DC" }, vocal: { value: "#3FA9F5" }, cueHot: { value: "#3CEB50" },
    cueHotText: { value: "#000000" }, cueHead: { value: "#EA3323" }, cueTeal: { value: "#10B176" },
    cueOrange: { value: "#FF8C00" }, cueBlue: { value: "#305AFF" }, cueYellow: { value: "#E1AA00" },
    cuePink: { value: "#F51E8C" }, cueAqua: { value: "#50B0F2" }, cueLime: { value: "#9BD723" },
    cuePurple: { value: "#AA72FF" },
  },
} as const;

export default theme;
