import { useEffect, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import alphaRingLogo from "./assets/logo_alpharing.png";
import johnHalo from "./assets/john_halo.png";
import background from "./assets/halo_ce.png";
import "./App.css";

const MENU_BUTTONS = [
  { label: "Play MCC with Anti-Cheat", action: () => console.log("Button 1") },
  { label: "Play Splitscreen Halo", action: () => console.log("Button 2") },
  {
    label: "Quit to Desktop",
    action: () => getCurrentWindow().close(),
  },
];

const GAMEPAD_AXIS_DEADZONE = 0.5;

function App({buildInfo = "", modInfo = ""}) {
  const [selectedIndex, setSelectedIndex] = useState(0);
  const selectedIndexRef = useRef(selectedIndex);
  selectedIndexRef.current = selectedIndex;

  const moveSelection = (delta) => {
    setSelectedIndex((prev) => (prev + delta + MENU_BUTTONS.length) % MENU_BUTTONS.length);
  };

  const activateSelection = () => {
    MENU_BUTTONS[selectedIndexRef.current].action();
  };

  useEffect(() => {
    const handleKeyDown = (e) => {
      switch (e.key) {
        case "ArrowUp":
        case "ArrowLeft":
        case "w":
        case "W":
        case "a":
        case "A":
          e.preventDefault();
          moveSelection(-1);
          break;
        case "ArrowDown":
        case "ArrowRight":
        case "s":
        case "S":
        case "d":
        case "D":
          e.preventDefault();
          moveSelection(1);
          break;
        case "Enter":
        case " ":
          e.preventDefault();
          activateSelection();
          break;
        default:
          break;
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, []);

  useEffect(() => {
    let rafId;
    const wasHeld = { prev: false, next: false, confirm: false };

    const poll = () => {
      const pads = navigator.getGamepads ? navigator.getGamepads() : [];
      const gamepad = Array.prototype.find.call(pads, Boolean);

      if (gamepad) {
        const axisX = gamepad.axes[0] ?? 0;
        const axisY = gamepad.axes[1] ?? 0;
        const dpadUp = gamepad.buttons[12]?.pressed ?? false;
        const dpadDown = gamepad.buttons[13]?.pressed ?? false;
        const dpadLeft = gamepad.buttons[14]?.pressed ?? false;
        const dpadRight = gamepad.buttons[15]?.pressed ?? false;
        const faceDown = gamepad.buttons[0]?.pressed ?? false;

        const prevHeld = dpadUp || dpadLeft || axisY < -GAMEPAD_AXIS_DEADZONE || axisX < -GAMEPAD_AXIS_DEADZONE;
        const nextHeld = dpadDown || dpadRight || axisY > GAMEPAD_AXIS_DEADZONE || axisX > GAMEPAD_AXIS_DEADZONE;

        if (prevHeld && !wasHeld.prev) moveSelection(-1);
        if (nextHeld && !wasHeld.next) moveSelection(1);
        if (faceDown && !wasHeld.confirm) activateSelection();

        wasHeld.prev = prevHeld;
        wasHeld.next = nextHeld;
        wasHeld.confirm = faceDown;
      }

      rafId = requestAnimationFrame(poll);
    };

    rafId = requestAnimationFrame(poll);
    return () => cancelAnimationFrame(rafId);
  }, []);

  return (
    <main className="container">
      <div className="background" style={{ backgroundImage: `url(${background})` }}>
        <img className="logo" src={alphaRingLogo}/>
        <img className="mc" src={johnHalo}/>
        <div className="menu">
          {MENU_BUTTONS.map((button, index) => (
            <button
              key={button.label}
              type="button"
              className={`menu-button${index === selectedIndex ? " selected" : ""}`}
              onMouseEnter={() => setSelectedIndex(index)}
              onClick={() => {
                setSelectedIndex(index);
                button.action();
              }}
            >
              {button.label}
            </button>
          ))}
        </div>
          <p className="info">{buildInfo} {modInfo}</p>
      </div>
    </main>
  );
}

export default App;
