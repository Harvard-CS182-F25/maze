from maze_core._core import run, run_headless, parse_config, GameState, GameResult, Action, AgentState, Raycast, MazeConfig, AgentConfig, FlagConfig, MazeGenerationConfig, OccupancyGrid, OccupancyGridCell, EntityType, SensorConfidence

from typing import Protocol, runtime_checkable

Position = tuple[float, float]
Velocity = tuple[float, float]

@runtime_checkable
class AgentProtocol(Protocol):
    def __init__(self) -> None: ...

    def get_action(self, game_state: GameState, occupancy_grid: OccupancyGrid, dt: float) -> Action:
        """Called once per policy tick.

        `occupancy_grid` is the grid the agent writes its map into. `dt` is the elapsed simulated
        time since the previous query. Pausing stops calls; a slow policy pauses simulation.
        """
        ...

    @property
    def estimated_position(self) -> Position:
        """The agent's own estimate of where it is.

        Optional: agents that do not estimate their position may leave this unimplemented, in which
        case the translucent estimated-position marker is simply not drawn.
        """
        ...

__all__ = ["run", "run_headless", "parse_config", "GameState", "GameResult", "Action", "AgentState", "Raycast", "AgentProtocol", "MazeConfig", "AgentConfig", "FlagConfig", "MazeGenerationConfig", "OccupancyGrid", "OccupancyGridCell", "EntityType", "Position", "Velocity", "SensorConfidence"]
