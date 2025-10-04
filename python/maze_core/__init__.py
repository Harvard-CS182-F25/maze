from maze_core._core import run, parse_config, GameState, Action, AgentState, HitInfo, MazeConfig, AgentConfig, FlagConfig, CapturePointConfig, CameraConfig

from typing import Protocol, runtime_checkable

Position = tuple[float, float]
Velocity = tuple[float, float]

@runtime_checkable
class AgentProtocol(Protocol):
    def __init__(self) -> None: ...

    def startup(self, initial_state: GameState) -> None: ...

    def get_action(self, game_state: GameState) -> Action: ...

__all__ = ["run", "parse_config", "GameState", "Action", "AgentState", "HitInfo", "AgentProtocol", "MazeConfig", "AgentConfig", "FlagConfig", "CapturePointConfig", "CameraConfig"]