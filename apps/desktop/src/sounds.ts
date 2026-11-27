// Original short earcons synthesized with Web Audio (no audio files to license).
// Start: rising two-note blip. Stop: falling blip. Error: low double pulse. Each < 150 ms.

let ctx: AudioContext | null = null;

function tone(at: number, freq: number, dur: number, gain = 0.08) {
  if (!ctx) ctx = new AudioContext();
  const osc = ctx.createOscillator();
  const g = ctx.createGain();
  osc.type = "sine";
  osc.frequency.setValueAtTime(freq, ctx.currentTime + at);
  g.gain.setValueAtTime(0, ctx.currentTime + at);
  g.gain.linearRampToValueAtTime(gain, ctx.currentTime + at + 0.008);
  g.gain.exponentialRampToValueAtTime(0.0001, ctx.currentTime + at + dur);
  osc.connect(g).connect(ctx.destination);
  osc.start(ctx.currentTime + at);
  osc.stop(ctx.currentTime + at + dur + 0.02);
}

export function play(name: string) {
  switch (name) {
    case "start":
      tone(0, 660, 0.06);
      tone(0.055, 990, 0.07);
      break;
    case "stop":
      tone(0, 880, 0.06);
      tone(0.055, 587, 0.07);
      break;
    case "error":
      tone(0, 220, 0.05, 0.1);
      tone(0.08, 220, 0.05, 0.1);
      break;
  }
}
