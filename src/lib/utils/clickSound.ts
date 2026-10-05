let ctx: AudioContext | null = null;

export function playClickSound() {
  if (!ctx) ctx = new AudioContext();
  const buf = ctx.createBuffer(1, 800, ctx.sampleRate);
  const data = buf.getChannelData(0);
  for (let i = 0; i < data.length; i++) {
    const t = i / ctx.sampleRate;
    data[i] =
      Math.exp(-t * 80) * (Math.sin(t * 3000) * 0.3 + Math.sin(t * 6000) * 0.1);
  }
  const src = ctx.createBufferSource();
  src.buffer = buf;
  const gain = ctx.createGain();
  gain.gain.value = 0.15;
  src.connect(gain).connect(ctx.destination);
  src.start();
}
