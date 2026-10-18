{$mode objfpc}{$H+}

unit uMCP;

{
  uMCP — Minimal MCP client for Free Pascal / Lazarus.
  Spawns the firebird-5-mcp binary via POSIX pipes (fork/exec),
  sends a JSON-RPC request via stdin, and returns the text result from stdout.

  Uses fpjson (FPC 3.2.2+) for JSON parsing.
  No TProcess dependency — works on FPC 3.2.2 and later.
}

interface

uses
  SysUtils, fpjson, jsonparser;

const
  MCP_BINARY  = '/home/edujed/.local/bin/firebird-5-mcp';

/// Calls an MCP tool and returns the text result.
function MCPCall(const AToolName, AArgsJSON: string): string;

/// Lists all available MCP tools (calls "tools/list").
function MCPListTools: string;

implementation

uses
  Classes;

type
  TFDArray = array[0..1] of Integer;

const
  STDIN  = 0;
  STDOUT = 1;
  STDERR = 2;

// --- POSIX wrappers (declared as external C functions) ---
function pipe(var apipes: TFDArray): Integer; cdecl; external 'libc';
function fork: LongInt; cdecl; external 'libc';
function execv(const aPath, aArgs: PChar): Integer; cdecl; external 'libc';
function dup2(aOldFd, aNewFd: Integer): Integer; cdecl; external 'libc';
function close(aFd: Integer): Integer; cdecl; external 'libc';
function read(aFd: Integer; var aBuf; aCount: LongInt): LongInt; cdecl; external 'libc';
function write(aFd: Integer; const aBuf; aCount: LongInt): LongInt; cdecl; external 'libc';
function waitpid(aPid: LongInt; var aStatus: Integer; aOptions: Integer): LongInt; cdecl; external 'libc';

// --- Internal helper: execute MCP binary with a JSON-RPC request ---
function ExecuteMCP(const ARequest: string): string;
var
  pipeIn, pipeOut: TFDArray;
  pid: LongInt;
  status: Integer;
  buf: array[0..65535] of Char;
  n: LongInt;
  reqBytes: PChar;
  args: array[0..1] of PChar;
begin
  Result := '';

  // Create pipes: pipeIn[1] = write end (parent writes request),
  //               pipeOut[0] = read end (parent reads response)
  if pipe(pipeIn) <> 0 then
  begin
    Result := 'ERROR: pipe() failed';
    Exit;
  end;
  if pipe(pipeOut) <> 0 then
  begin
    close(pipeIn[0]);
    close(pipeIn[1]);
    Result := 'ERROR: pipe() failed (output)';
    Exit;
  end;

  pid := fork;
  if pid < 0 then
  begin
    close(pipeIn[0]); close(pipeIn[1]);
    close(pipeOut[0]); close(pipeOut[1]);
    Result := 'ERROR: fork() failed';
    Exit;
  end;

  if pid = 0 then
  begin
    // === CHILD PROCESS ===
    // stdin  <- pipeIn[0]
    dup2(pipeIn[0], STDIN);
    // stdout -> pipeOut[1]
    dup2(pipeOut[1], STDOUT);
    // stderr -> /dev/null (suppress)
    close(pipeIn[0]);
    close(pipeIn[1]);
    close(pipeOut[0]);
    close(pipeOut[1]);

    args[0] := MCP_BINARY;
    args[1] := nil;
    execv(MCP_BINARY, @args);
    // If execv returns, it failed
    close(STDERR);
    // Exit child
    Halt(127);
  end;

  // === PARENT PROCESS ===
  // Close unused pipe ends
  close(pipeOut[1]);  // parent doesn't write to output pipe
  close(pipeIn[0]);   // parent doesn't read from input pipe

  // Write the JSON-RPC request to the child's stdin
  reqBytes := PChar(ARequest);
  n := 0;
  while n < Length(ARequest) do
  begin
    n := write(pipeIn[1], reqBytes[n], Length(ARequest) - n);
    if n <= 0 then Break;
  end;
  close(pipeIn[1]);  // Close write end to signal EOF to child

  // Read all output from the child's stdout
  while True do
  begin
    n := read(pipeOut[0], buf, SizeOf(buf));
    if n <= 0 then Break;
    SetLength(Result, Length(Result) + n);
    Move(buf, Result[Length(Result) - n + 1], n);
  end;
  close(pipeOut[0]);

  // Wait for child to exit
  waitpid(pid, status, 0);
end;

function MCPCall(const AToolName, AArgsJSON: string): string;
var
  Request, RawResponse: string;
  Data: TJSONData;
  Obj: TJSONObject;
  Arr: TJSONArray;
  Item: TJSONData;
  TextObj: TJSONObject;
begin
  Result := '';
  Request := Format(
    '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"%s","arguments":%s}}',
    [AToolName, AArgsJSON]);

  RawResponse := ExecuteMCP(Request);

  if RawResponse <> '' then
  begin
    try
      Data := GetJSON(RawResponse);
      try
        if Data is TJSONObject then
        begin
          Obj := TJSONObject(Data);
          Arr := TJSONArray(Obj.FindPath('result.content'));
          if (Arr <> nil) and (Arr.Count > 0) then
          begin
            Item := Arr.Items[0];
            if Item is TJSONObject then
            begin
              TextObj := TJSONObject(Item);
              if TextObj.FindPath('text') <> nil then
                Result := TextObj.FindPath('text').AsString;
            end;
          end;
        end;
      finally
        Data.Free;
      end;
    except
    end;
  end;

  if Result = '' then
    Result := 'ERROR: empty response. Raw: ' + RawResponse;
end;

function MCPListTools: string;
var
  Request, RawResponse: string;
  Data: TJSONData;
  Obj: TJSONObject;
  Arr: TJSONArray;
  Item: TJSONData;
  ToolObj: TJSONObject;
  L: TStringList;
  I: Integer;
begin
  Result := '';
  Request := '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}';

  RawResponse := ExecuteMCP(Request);

  L := TStringList.Create;
  try
    if RawResponse <> '' then
    begin
      try
        Data := GetJSON(RawResponse);
        try
          if Data is TJSONObject then
          begin
            Obj := TJSONObject(Data);
            Arr := TJSONArray(Obj.FindPath('result.tools'));
            if Arr <> nil then
            begin
              for I := 0 to Arr.Count - 1 do
              begin
                Item := Arr.Items[I];
                if Item is TJSONObject then
                begin
                  ToolObj := TJSONObject(Item);
                  L.Add(Format('  %s — %s',
                    [ToolObj.FindPath('name').AsString,
                     ToolObj.FindPath('description').AsString]));
                end;
              end;
              Result := Format('%d tools available:%s%s', [Arr.Count, LineEnding, L.Text]);
            end;
          end;
        finally
          Data.Free;
        end;
      except
      end;
    end;
  finally
    L.Free;
  end;

  if Result = '' then
    Result := 'ERROR: empty response. Raw: ' + RawResponse;
end;

end.
