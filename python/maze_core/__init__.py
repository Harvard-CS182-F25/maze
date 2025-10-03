from maze_core._core import run, GameState, Action, AgentState, HitInfo

from typing import Protocol, runtime_checkable

Position = tuple[float, float]
Velocity = tuple[float, float]

@runtime_checkable
class AgentProtocol(Protocol):
    def __init__(self) -> None: ...

    def startup(self, initial_state: GameState) -> None: ...

    def get_action(self, game_state: GameState) -> Action: ...

__all__ = ["run", "GameState", "Action", "AgentState", "HitInfo", "AgentProtocol"]