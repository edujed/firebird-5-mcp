program test_events;

{
  test_events — Free Pascal example demonstrating Firebird POST_EVENT.

  Connects to the test-mcp database and spawns two threads:
    1. Listener thread — registers for 'user_changed' and 'user_updated'
       events via TFBEventMonitor, loops printing each event as it arrives.
    2. Worker thread — performs INSERTs and UPDATEs on the USERS table.
       The database triggers (TG_USER_EVENT, TG_USER_UPDATE_EVENT) fire
       POST_EVENT on commit, which the listener receives.

  This demonstrates the same mechanism used by the MCP server's
  on_success_event / on_fail_event params: the MCP server executes
  POST_EVENT '<name>' in a fresh transaction, and any client registered
  for that event name (via SET EVENT) receives the notification on commit.

  Build:
    fpc -otest_events test_events.pas

  Run:
    ./test_events

  Prerequisites:
    - test-mcp database created and populated (see examples/test-mcp/)
    - Firebird 5 server running
    - FPC 3.2.2+ with fcl-db package
}

{$mode objfpc}{$H+}

uses
  {$IFDEF UNIX}
  cthreads,
  {$ENDIF}
  SysUtils, Classes,
  ibconnection, sqldb, fbeventmonitor;

const
  DB_HOST = 'localhost';
  DB_PATH = '/home/edujed/.firebird-mcp/test-mcp.fdb';
  DB_USER = 'SYSDBA';
  DB_PASS = 'masterkey';

type
  { TEventListener — manages the event callback }
  TEventListener = class
  public
    procedure OnEventAlert(Sender: TObject; EventName: string;
      EventCount: LongInt; var CancelAlerts: Boolean);
  end;

procedure TEventListener.OnEventAlert(Sender: TObject; EventName: string;
  EventCount: LongInt; var CancelAlerts: Boolean);
begin
  Writeln(Format('[listener] EVENT: %s (count=%d) at %s',
    [EventName, EventCount, TimeToStr(Now)]));
end;

var
  ListenerDone: Boolean;
  ListenerTerminate: Boolean;

procedure ListenerThread(AThread: TThread);
var
  Conn: TIBConnection;
  Monitor: TFBEventMonitor;
  Listener: TEventListener;
begin
  try
    Conn := TIBConnection.Create(nil);
    Monitor := TFBEventMonitor.Create(nil);
    Listener := TEventListener.Create;
    try
      Conn.HostName := DB_HOST;
      Conn.DatabaseName := DB_PATH;
      Conn.UserName := DB_USER;
      Conn.Password := DB_PASS;
      Conn.Connected := True;

      Writeln('[listener] Connected to database');
      Writeln('[listener] Registering for events: user_changed, user_updated');

      Monitor.Connection := Conn;
      Monitor.Events.CommaText := 'user_changed,user_updated';
      Monitor.OnEventAlert := @Listener.OnEventAlert;
      Monitor.RegisterEvents;

      Writeln('[listener] Waiting for events...');

      // Keep the thread alive until terminated
      while not ListenerTerminate do
      begin
        Sleep(100);
      end;

      Monitor.UnRegisterEvents;
    finally
      Listener.Free;
      Monitor.Free;
      Conn.Free;
    end;
  except
    on E: Exception do
    begin
      Writeln(Format('[listener] Fatal error: %s', [E.Message]));
    end;
  end;
  ListenerDone := True;
end;

procedure WorkerThread(AThread: TThread);
var
  Conn: TIBConnection;
  Trans: TSQLTransaction;
  Query: TSQLQuery;
  I: Integer;
begin
  try
    Conn := TIBConnection.Create(nil);
    Trans := TSQLTransaction.Create(nil);
    Query := TSQLQuery.Create(nil);
    try
      Trans.Connection := Conn;
      Query.Connection := Conn;
      Query.Transaction := Trans;

      Conn.HostName := DB_HOST;
      Conn.DatabaseName := DB_PATH;
      Conn.UserName := DB_USER;
      Conn.Password := DB_PASS;
      Conn.Connected := True;

      Writeln('[worker] Connected to database');

      // Give the listener time to register
      Sleep(2000);

      // INSERT 3 new users (triggers POST_EVENT 'user_changed' on commit)
      for I := 900 to 902 do
      begin
        Trans.BeginTransaction;
        try
          Query.SQL.Clear;
          Query.SQL.Add(Format(
            'INSERT INTO USERS (ID, DEPARTMENT_ID, NAME, EMAIL, PASSWORD_HASH, ACTIVE, SALARY, HIRED_AT, CREATED_AT, UPDATED_AT) ' +
            'VALUES (%d, 1, :name, :email, :hash, TRUE, 50000, DATE ''2024-01-01'', :now, :now)',
            [I]));
          Query.ParamByName('name').AsString := Format('Test User %d', [I]);
          Query.ParamByName('email').AsString := Format('test%d@company.com', [I]);
          Query.ParamByName('hash').AsString := Format('hash_%d', [I]);
          Query.ParamByName('now').AsDateTime := Now;
          Query.ExecSQL;
          Trans.Commit;
          Writeln(Format('[worker] INSERT user %d committed', [I]));
        except
          on E: Exception do
          begin
            Trans.Rollback;
            Writeln(Format('[worker] INSERT error: %s', [E.Message]));
          end;
        end;
        Sleep(500);
      end;

      // UPDATE 2 users (triggers POST_EVENT 'user_updated' on commit)
      for I := 900 to 901 do
      begin
        Trans.BeginTransaction;
        try
          Query.SQL.Clear;
          Query.SQL.Add('UPDATE USERS SET SALARY = :salary, UPDATED_AT = :now WHERE ID = :id');
          Query.ParamByName('salary').AsFloat := 60000 + I;
          Query.ParamByName('now').AsDateTime := Now;
          Query.ParamByName('id').AsInteger := I;
          Query.ExecSQL;
          Trans.Commit;
          Writeln(Format('[worker] UPDATE user %d committed', [I]));
        except
          on E: Exception do
          begin
            Trans.Rollback;
            Writeln(Format('[worker] UPDATE error: %s', [E.Message]));
          end;
        end;
        Sleep(500);
      end;

      Writeln('[worker] All operations completed');
    finally
      Query.Free;
      Trans.Free;
      Conn.Free;
    end;
  except
    on E: Exception do
    begin
      Writeln(Format('[worker] Fatal error: %s', [E.Message]));
    end;
  end;
end;

{ TThread subclass for listener }
type
  TListenerThread = class(TThread)
  public
    procedure Execute; override;
  end;

procedure TListenerThread.Execute;
begin
  ListenerThread(Self);
end;

{ TThread subclass for worker }
type
  TWorkerThread = class(TThread)
  public
    procedure Execute; override;
  end;

procedure TWorkerThread.Execute;
begin
  WorkerThread(Self);
end;

var
  Listener: TListenerThread;
  Worker: TWorkerThread;
begin
  Writeln('firebird-5-mcp — POST_EVENT Example');
  Writeln('===================================');
  Writeln;
  Writeln('This example demonstrates Firebird''s POST_EVENT mechanism:');
  Writeln('  1. A listener thread registers for events via TFBEventMonitor');
  Writeln('  2. A worker thread performs INSERTs and UPDATEs');
  Writeln('  3. Database triggers fire POST_EVENT on commit');
  Writeln('  4. The listener receives and prints each event');
  Writeln;
  Writeln('The MCP server uses the same mechanism: when a tool call includes');
  Writeln('on_success_event or on_fail_event, the server executes POST_EVENT');
  Writeln('in a fresh transaction, and any registered client receives it.');
  Writeln;

  Listener := TListenerThread.Create(False);
  Worker := TWorkerThread.Create(False);

  Worker.Start;
  Listener.Start;

  Worker.WaitFor;
  Sleep(2000);  // Give listener time to print remaining events
  ListenerTerminate := True;
  Listener.WaitFor;

  Writeln;
  Writeln('Done. Check the output above for received events.');
end.
