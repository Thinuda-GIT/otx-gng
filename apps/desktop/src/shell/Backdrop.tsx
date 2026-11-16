/** Ambient backdrop: slow blurred gradient orbs behind all glass. Reacts to load via --load
 *  (set in App) and to the per-game tint via --game-tint. Animations are transform-only. */
export function Backdrop() {
  return (
    <div className="otx-backdrop" aria-hidden>
      <div className="otx-orb otx-orb--green" />
      <div className="otx-orb otx-orb--cyan" />
      <div className="otx-orb otx-orb--violet" />
      <div className="otx-orb otx-orb--red" />
      <div className="otx-orb otx-orb--tint" />
      <div className="otx-grid" />
    </div>
  );
}
